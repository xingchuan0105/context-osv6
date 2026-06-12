use std::collections::HashMap;

use ingestion::parser::{
    PdfPageBackend, PdfParser, PdfParsePlan, VisualPdfParser,
};
use ingestion::{
    DocumentIr, DocumentType, IngestionError, PageParseStatus, PageStatusEntry, ParseBackend,
};
use uuid::Uuid;

use super::b_class::enrich_b_class_figures;
use super::context::PdfParseContext;
use super::merge::{document_ir_from_parsed_document, merge_pdf_ir, DocumentIrPdfExt};
use super::paddle::execute_paddle_ocr;

pub async fn execute_pdf_parse(
    ctx: &PdfParseContext,
    bytes: &[u8],
    filename: &str,
    _object_path: &str,
    document_id: Uuid,
    plan: &PdfParsePlan,
) -> Result<DocumentIr, IngestionError> {
    let edge_pages: Vec<u32> = plan
        .pages
        .iter()
        .filter(|p| p.backend == PdfPageBackend::EdgeParse)
        .map(|p| p.page_number)
        .collect();
    let paddle_pages: Vec<u32> = plan
        .pages
        .iter()
        .filter(|p| p.backend == PdfPageBackend::PaddleOcr)
        .map(|p| p.page_number)
        .collect();
    let visual_pages: Vec<u32> = plan
        .pages
        .iter()
        .filter(|p| p.backend == PdfPageBackend::VisualRaster)
        .map(|p| p.page_number)
        .collect();

    let mut page_durations: HashMap<u32, u64> = HashMap::new();

    let digital_ir = if edge_pages.is_empty() {
        None
    } else {
        let ep_start = std::time::Instant::now();
        let parsed = PdfParser
            .parse_pages(bytes, filename, &edge_pages)
            .await
            .map_err(|error| {
                IngestionError::StateSink(format!(
                    "pdf digital parse failed for {filename}: {error}"
                ))
            })?;
        let ep_elapsed = ep_start.elapsed().as_millis() as u64;
        let ep_per_page = ep_elapsed / edge_pages.len() as u64;
        for &pn in &edge_pages {
            page_durations.insert(pn, ep_per_page);
        }
        Some(
            document_ir_from_parsed_document(
                document_id,
                filename,
                DocumentType::Pdf,
                ParseBackend::EdgeParsePdf,
                parsed,
            )
            .with_pdf_defaults(ParseBackend::EdgeParsePdf),
        )
    };

    let (paddle_ir, paddle_successful) = if paddle_pages.is_empty() {
        (None, std::collections::HashSet::new())
    } else {
        let pp_start = std::time::Instant::now();
        match execute_paddle_ocr(bytes, filename, document_id, &paddle_pages).await {
            Ok((ir, pages)) => {
                let pp_elapsed = pp_start.elapsed().as_millis() as u64;
                let pp_per_page = pp_elapsed / paddle_pages.len() as u64;
                for &pn in &paddle_pages {
                    page_durations.insert(pn, pp_per_page);
                }
                (Some(ir), pages)
            }
            Err(e) => {
                tracing::warn!(filename, error = %e, "PaddleOCR failed, falling back to VisualRaster for OCR pages");
                (None, std::collections::HashSet::new())
            }
        }
    };

    let paddle_needs_fallback: Vec<u32> = if paddle_pages.is_empty() {
        Vec::new()
    } else {
        paddle_pages
            .iter()
            .filter(|p| !paddle_successful.contains(p))
            .copied()
            .collect()
    };
    let effective_visual_pages = if paddle_needs_fallback.is_empty() {
        visual_pages.clone()
    } else {
        let mut pages = visual_pages;
        pages.extend(&paddle_needs_fallback);
        pages.sort();
        pages
    };

    let visual_ir = if effective_visual_pages.is_empty() {
        None
    } else {
        let renderer = ctx.pdf_renderer_client.as_ref().ok_or_else(|| {
            IngestionError::StateSink(format!(
                "PDF visual raster selected for {filename}, but PDF_RENDERER_BASE_URL is not configured"
            ))
        })?;
        let parser = VisualPdfParser::new(renderer.clone());
        let vr_start = std::time::Instant::now();
        let result = parser
            .parse_pages(bytes, filename, document_id, &effective_visual_pages)
            .await
            .map_err(|error| {
                IngestionError::StateSink(format!(
                    "visual pdf parse failed for {filename}: {error}"
                ))
            })?;
        let vr_elapsed = vr_start.elapsed().as_millis() as u64;
        let vr_per_page = vr_elapsed / effective_visual_pages.len() as u64;
        for &pn in &effective_visual_pages {
            page_durations.insert(pn, vr_per_page);
        }
        Some(result)
    };

    let mut merged = merge_pdf_ir(
        document_id,
        filename,
        plan,
        digital_ir,
        paddle_ir,
        visual_ir,
        &paddle_successful,
    )?;

    let page_status: Vec<PageStatusEntry> = plan
        .pages
        .iter()
        .map(|p| {
            let route = p.reason.route_label();
            let actual_page = merged.pages.iter().find(|mp| mp.page_number == p.page_number);
            let status = if p.backend == PdfPageBackend::PaddleOcr {
                match actual_page {
                    Some(pg) if pg.backend == ParseBackend::PaddleOcrPdf => PageParseStatus::Ok,
                    Some(_) => PageParseStatus::Partial,
                    None => PageParseStatus::OcrFail,
                }
            } else if actual_page.is_some() {
                PageParseStatus::Ok
            } else {
                PageParseStatus::Missing
            };
            PageStatusEntry {
                page_no: p.page_number,
                status,
                route: route.to_string(),
                duration_ms: page_durations.get(&p.page_number).copied(),
            }
        })
        .collect();
    merged.metadata.insert(
        "page_status".to_string(),
        serde_json::to_string(&page_status).unwrap_or_default(),
    );

    enrich_b_class_figures(ctx, bytes, &mut merged, plan).await;

    Ok(merged)
}
