use std::collections::HashSet;

use ingestion::parser::{
    normalize_parsed_document, pdf_page_route_labels, PageRouteKind, PdfPageBackend, PdfPagePlan,
    PdfParsePlan, ParsedDocument,
};
use ingestion::{
    BlockIr, BlockType, DocumentIr, DocumentType, IngestionError, PageIr, ParseBackend,
};
use uuid::Uuid;

pub fn document_ir_from_parsed_document(
    document_id: Uuid,
    filename: &str,
    doc_type: DocumentType,
    backend: ParseBackend,
    parsed: ParsedDocument,
) -> DocumentIr {
    let normalized = normalize_parsed_document(&parsed, backend.as_str());
    let mut document_ir = DocumentIr::from_normalized_document(
        document_id.to_string(),
        doc_type,
        backend,
        &normalized,
    );
    if document_ir.title.trim().is_empty() {
        document_ir.title = filename.to_string();
    }
    document_ir
}

pub fn filter_document_ir_to_page(document_ir: &DocumentIr, page_number: u32) -> DocumentIr {
    let mut filtered = DocumentIr::new(
        document_ir.document_id.clone(),
        document_ir.title.clone(),
        document_ir.doc_type.clone(),
        document_ir.primary_backend.clone(),
    );
    filtered.backend_version = document_ir.backend_version.clone();
    filtered.language = document_ir.language.clone();
    filtered.metadata = document_ir.metadata.clone();
    filtered.pages = document_ir
        .pages
        .iter()
        .filter(|page| page.page_number == page_number)
        .cloned()
        .collect();
    filtered.blocks = document_ir
        .blocks
        .iter()
        .filter(|block| {
            block.page == Some(page_number) || block.source_locator.page == Some(page_number)
        })
        .cloned()
        .collect();
    filtered.assets = document_ir
        .assets
        .iter()
        .filter(|asset| asset.page == Some(page_number))
        .cloned()
        .collect();
    filtered.warnings = document_ir
        .warnings
        .iter()
        .filter(|warning| warning.page == Some(page_number))
        .cloned()
        .collect();
    filtered
}

fn effective_route_kinds(page_plan: &PdfPagePlan) -> Vec<PageRouteKind> {
    if !page_plan.route_kinds.is_empty() {
        return page_plan.route_kinds.clone();
    }
    match page_plan.backend {
        PdfPageBackend::EdgeParse => vec![PageRouteKind::Text],
        PdfPageBackend::PaddleOcr => vec![PageRouteKind::ScanOcr],
        PdfPageBackend::VisualRaster => vec![],
    }
}

fn wants_digital(kinds: &[PageRouteKind]) -> bool {
    kinds
        .iter()
        .any(|k| matches!(k, PageRouteKind::Text | PageRouteKind::Figure))
}

fn wants_paddle_ocr(kinds: &[PageRouteKind]) -> bool {
    kinds
        .iter()
        .any(|k| matches!(k, PageRouteKind::TableOcr | PageRouteKind::ScanOcr))
}

fn is_liteparse_ir(ir: &DocumentIr) -> bool {
    ir.primary_backend == ParseBackend::LiteParsePdf
}

fn digital_backend_for(ir: Option<&DocumentIr>) -> ParseBackend {
    if ir.is_some_and(is_liteparse_ir) {
        ParseBackend::LiteParsePdf
    } else {
        // Pre-P4 stored IR may still carry `edge_parse_pdf`.
        ParseBackend::EdgeParsePdf
    }
}

fn paddle_has_text_on_page(ir: &DocumentIr, page_number: u32) -> bool {
    ir.blocks.iter().any(|b| {
        b.page == Some(page_number)
            && matches!(
                b.block_type,
                BlockType::Paragraph | BlockType::Heading | BlockType::ListItem
            )
            && !b.text.trim().is_empty()
    })
}

fn upsert_page_row(merged: &mut DocumentIr, page_number: u32, incoming: PageIr) {
    if let Some(existing) = merged.pages.iter_mut().find(|p| p.page_number == page_number) {
        existing.text_char_count = existing
            .text_char_count
            .saturating_add(incoming.text_char_count);
        existing.image_count = existing.image_count.saturating_add(incoming.image_count);
        if incoming.width.is_some() {
            existing.width = incoming.width;
        }
        if incoming.height.is_some() {
            existing.height = incoming.height;
        }
    } else {
        merged.pages.push(incoming);
    }
}

fn set_merged_page_backend(merged: &mut DocumentIr, page_number: u32, backend: ParseBackend) {
    if let Some(page) = merged.pages.iter_mut().find(|p| p.page_number == page_number) {
        page.backend = backend;
    }
}

fn append_page_content(
    merged: &mut DocumentIr,
    page_number: u32,
    page_data: DocumentIr,
    block_backend: ParseBackend,
    skip_text_blocks: bool,
) {
    let mut page_row = page_data.pages.into_iter().next().unwrap_or(PageIr {
        page_number,
        width: None,
        height: None,
        backend: block_backend.clone(),
        text_char_count: 0,
        image_count: 0,
        metadata: Default::default(),
    });
    page_row.page_number = page_number;
    page_row.backend = block_backend.clone();
    upsert_page_row(merged, page_number, page_row);

    for mut block in page_data.blocks {
        if skip_text_blocks && is_text_block(&block) {
            continue;
        }
        block.page = Some(page_number);
        block.source_locator.page = Some(page_number);
        block.parser_backend = block_backend.clone();
        merged.blocks.push(block);
    }
    for mut asset in page_data.assets {
        asset.page = Some(page_number);
        asset.parser_backend = block_backend.clone();
        merged.assets.push(asset);
    }
}

fn is_text_block(block: &BlockIr) -> bool {
    matches!(
        block.block_type,
        BlockType::Paragraph | BlockType::Table | BlockType::Heading | BlockType::ListItem
    )
}

fn resolve_page_backend(
    kinds: &[PageRouteKind],
    paddle_applied: bool,
    digital_applied: bool,
    visual_applied: bool,
    digital_backend: ParseBackend,
) -> ParseBackend {
    if paddle_applied {
        ParseBackend::PaddleOcrPdf
    } else if visual_applied {
        ParseBackend::VisualRasterPdf
    } else if digital_applied {
        digital_backend
    } else if wants_paddle_ocr(kinds) {
        ParseBackend::VisualRasterPdf
    } else {
        digital_backend
    }
}

fn merge_page_by_route_kinds(
    merged: &mut DocumentIr,
    page_plan: &PdfPagePlan,
    digital_ir: Option<&DocumentIr>,
    paddle_ir: Option<&DocumentIr>,
    visual_ir: Option<&DocumentIr>,
    paddle_successful: &HashSet<u32>,
) {
    let kinds = effective_route_kinds(page_plan);
    let page_number = page_plan.page_number;
    let digital_backend = digital_backend_for(digital_ir);

    let mut paddle_applied = false;
    let mut digital_applied = false;
    let mut visual_applied = false;

    if wants_paddle_ocr(&kinds)
        && paddle_ir.is_some()
        && paddle_successful.contains(&page_number)
    {
        let page_data = filter_document_ir_to_page(paddle_ir.unwrap(), page_number);
        if !page_data.blocks.is_empty() || !page_data.pages.is_empty() {
            append_page_content(
                merged,
                page_number,
                page_data,
                ParseBackend::PaddleOcrPdf,
                false,
            );
            paddle_applied = true;
        }
    }

    if wants_digital(&kinds) && digital_ir.is_some() {
        let skip_text = paddle_applied && paddle_has_text_on_page(paddle_ir.unwrap(), page_number);
        let page_data = filter_document_ir_to_page(digital_ir.unwrap(), page_number);
        if !page_data.blocks.is_empty() || !page_data.pages.is_empty() {
            append_page_content(
                merged,
                page_number,
                page_data,
                digital_backend.clone(),
                skip_text,
            );
            digital_applied = true;
        }
    }

    let needs_visual = wants_paddle_ocr(&kinds) && !paddle_applied;
    if needs_visual {
        if let Some(visual) = visual_ir {
            let page_data = filter_document_ir_to_page(visual, page_number);
            if !page_data.blocks.is_empty() || !page_data.pages.is_empty() {
                append_page_content(
                    merged,
                    page_number,
                    page_data,
                    ParseBackend::VisualRasterPdf,
                    false,
                );
                visual_applied = true;
            }
        } else if !paddle_applied && !digital_applied && digital_ir.is_some() {
            // Degraded digital-only content for OCR pages when paddle/visual absent.
            let page_data = filter_document_ir_to_page(digital_ir.unwrap(), page_number);
            if !page_data.blocks.is_empty() {
                append_page_content(
                    merged,
                    page_number,
                    page_data,
                    digital_backend.clone(),
                    false,
                );
                digital_applied = true;
            }
        }
    }

    if page_plan.backend == PdfPageBackend::VisualRaster && !visual_applied {
        if let Some(visual) = visual_ir {
            let page_data = filter_document_ir_to_page(visual, page_number);
            append_page_content(
                merged,
                page_number,
                page_data,
                ParseBackend::VisualRasterPdf,
                false,
            );
            visual_applied = true;
        }
    }

    if merged.pages.iter().any(|p| p.page_number == page_number) {
        let backend = resolve_page_backend(
            &kinds,
            paddle_applied,
            digital_applied,
            visual_applied,
            digital_backend,
        );
        set_merged_page_backend(merged, page_number, backend);
    }
}

fn merge_page_legacy_backend(
    merged: &mut DocumentIr,
    page_plan: &PdfPagePlan,
    digital_ir: Option<&DocumentIr>,
    paddle_ir: Option<&DocumentIr>,
    visual_ir: Option<&DocumentIr>,
    paddle_successful: &HashSet<u32>,
) {
    // "Legacy" = historical wire enum names on `PdfPageBackend`, not a deprecated code path.
    let source_ir = match page_plan.backend {
        PdfPageBackend::EdgeParse => digital_ir,
        PdfPageBackend::PaddleOcr => {
            if paddle_ir.is_some() && paddle_successful.contains(&page_plan.page_number) {
                paddle_ir
            } else {
                visual_ir.or(digital_ir)
            }
        }
        PdfPageBackend::VisualRaster => visual_ir,
    };
    let page_backend = match page_plan.backend {
        PdfPageBackend::EdgeParse => digital_backend_for(digital_ir),
        PdfPageBackend::PaddleOcr => {
            if paddle_ir.is_some() && paddle_successful.contains(&page_plan.page_number) {
                ParseBackend::PaddleOcrPdf
            } else {
                ParseBackend::VisualRasterPdf
            }
        }
        PdfPageBackend::VisualRaster => ParseBackend::VisualRasterPdf,
    };

    let Some(source_ir) = source_ir else {
        return;
    };

    append_page_content(
        merged,
        page_plan.page_number,
        filter_document_ir_to_page(source_ir, page_plan.page_number),
        page_backend,
        false,
    );
}

pub fn merge_pdf_ir(
    document_id: Uuid,
    filename: &str,
    plan: &PdfParsePlan,
    digital_ir: Option<DocumentIr>,
    paddle_ir: Option<DocumentIr>,
    visual_ir: Option<DocumentIr>,
    paddle_successful: &HashSet<u32>,
) -> Result<DocumentIr, IngestionError> {
    let title = digital_ir
        .as_ref()
        .map(|d| d.title.clone())
        .filter(|t| !t.trim().is_empty())
        .or_else(|| {
            paddle_ir
                .as_ref()
                .map(|d| d.title.clone())
                .filter(|t| !t.trim().is_empty())
        })
        .or_else(|| {
            visual_ir
                .as_ref()
                .map(|d| d.title.clone())
                .filter(|t| !t.trim().is_empty())
        })
        .unwrap_or_else(|| filename.to_string());

    let has_edge = digital_ir.is_some();
    let has_paddle = paddle_ir.is_some();
    let has_visual = visual_ir.is_some();
    let backend_count = [has_edge, has_paddle, has_visual]
        .iter()
        .filter(|&&x| x)
        .count();

    let primary_backend = if has_edge {
        digital_backend_for(digital_ir.as_ref())
    } else if has_paddle {
        ParseBackend::PaddleOcrPdf
    } else {
        ParseBackend::VisualRasterPdf
    };

    let mut merged = DocumentIr::new(
        document_id.to_string(),
        title,
        DocumentType::Pdf,
        primary_backend,
    );

    if backend_count > 1 {
        let mode = if digital_ir.as_ref().is_some_and(is_liteparse_ir) {
            "liteparse_hybrid"
        } else {
            "hybrid_v2"
        };
        merged
            .metadata
            .insert("pdf_route_mode".to_string(), mode.to_string());
    } else if digital_ir.as_ref().is_some_and(is_liteparse_ir) {
        merged
            .metadata
            .insert("pdf_route_mode".to_string(), "liteparse_hybrid".to_string());
    }

    let mut page_routes_map = serde_json::Map::new();
    for p in &plan.pages {
        page_routes_map.insert(
            p.page_number.to_string(),
            serde_json::json!(pdf_page_route_labels(p)),
        );
    }
    merged.metadata.insert(
        "page_routes".to_string(),
        serde_json::to_string(&page_routes_map).unwrap_or_default(),
    );

    for ir in [&digital_ir, &paddle_ir, &visual_ir]
        .iter()
        .filter_map(|x| x.as_ref())
    {
        if merged.metadata.len() <= 2 {
            merged.metadata.extend(ir.metadata.clone());
        }
        merged.warnings.extend(ir.warnings.clone());
    }

    let use_route_kinds = plan.pages.iter().any(|p| !p.route_kinds.is_empty());

    for page_plan in &plan.pages {
        if use_route_kinds {
            merge_page_by_route_kinds(
                &mut merged,
                page_plan,
                digital_ir.as_ref(),
                paddle_ir.as_ref(),
                visual_ir.as_ref(),
                paddle_successful,
            );
        } else {
            merge_page_legacy_backend(
                &mut merged,
                page_plan,
                digital_ir.as_ref(),
                paddle_ir.as_ref(),
                visual_ir.as_ref(),
                paddle_successful,
            );
        }
    }

    Ok(merged)
}
