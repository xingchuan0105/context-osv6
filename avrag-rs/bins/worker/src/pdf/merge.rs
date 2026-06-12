use std::collections::HashSet;

use ingestion::parser::{normalize_parsed_document, PdfPageBackend, PdfParsePlan, ParsedDocument};
use ingestion::{
    DocumentIr, DocumentType, IngestionError, PageIr, ParseBackend,
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
        ParseBackend::EdgeParsePdf
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
        merged
            .metadata
            .insert("pdf_route_mode".to_string(), "hybrid_v2".to_string());
    }

    let mut page_routes_map = serde_json::Map::new();
    for p in &plan.pages {
        page_routes_map.insert(
            p.page_number.to_string(),
            serde_json::json!(p.reason.route_label()),
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
        if merged.metadata.len() <= 1 {
            merged.metadata.extend(ir.metadata.clone());
        }
        merged.warnings.extend(ir.warnings.clone());
    }

    for page_plan in &plan.pages {
        let source_ir = match page_plan.backend {
            PdfPageBackend::EdgeParse => digital_ir.as_ref(),
            PdfPageBackend::PaddleOcr => {
                if paddle_ir.is_some() && paddle_successful.contains(&page_plan.page_number) {
                    paddle_ir.as_ref()
                } else {
                    visual_ir.as_ref().or(digital_ir.as_ref())
                }
            }
            PdfPageBackend::VisualRaster => visual_ir.as_ref(),
        };
        let page_backend = match page_plan.backend {
            PdfPageBackend::EdgeParse => ParseBackend::EdgeParsePdf,
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
            continue;
        };

        let page_data = filter_document_ir_to_page(source_ir, page_plan.page_number);
        let mut page_row = page_data.pages.into_iter().next().unwrap_or(PageIr {
            page_number: page_plan.page_number,
            width: None,
            height: None,
            backend: page_backend.clone(),
            text_char_count: 0,
            image_count: 0,
            metadata: Default::default(),
        });
        page_row.page_number = page_plan.page_number;
        page_row.backend = page_backend.clone();
        merged.pages.push(page_row);

        merged
            .blocks
            .extend(page_data.blocks.into_iter().map(|mut block| {
                block.page = Some(page_plan.page_number);
                block.source_locator.page = Some(page_plan.page_number);
                block.parser_backend = page_backend.clone();
                block
            }));
        merged
            .assets
            .extend(page_data.assets.into_iter().map(|mut asset| {
                asset.page = Some(page_plan.page_number);
                asset.parser_backend = page_backend.clone();
                asset
            }));
    }

    Ok(merged)
}

pub trait DocumentIrPdfExt {
    fn with_pdf_defaults(self, backend: ParseBackend) -> Self;
}

impl DocumentIrPdfExt for DocumentIr {
    fn with_pdf_defaults(mut self, backend: ParseBackend) -> Self {
        self.doc_type = DocumentType::Pdf;
        self.primary_backend = backend.clone();
        for page in &mut self.pages {
            page.backend = backend.clone();
        }
        for block in &mut self.blocks {
            if block.page.is_none() {
                block.page = block.source_locator.page;
            }
            block.parser_backend = backend.clone();
        }
        for asset in &mut self.assets {
            asset.parser_backend = backend.clone();
        }
        self
    }
}
