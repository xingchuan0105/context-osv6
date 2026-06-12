use ingestion::{DocumentIr, ParseBackend};

use super::env::env_flag_enabled;
use super::types::StoredMultimodalChunk;

pub fn document_paddle_ocr_succeeded(document_ir: &DocumentIr) -> bool {
    if document_ir.metadata.get("ocr_backend").map(|s| s.as_str()) == Some("paddle") {
        return true;
    }
    document_ir
        .pages
        .iter()
        .any(|page| page.backend == ParseBackend::PaddleOcrPdf)
}

pub fn should_skip_page_raster_multimodal_index(
    chunk: &StoredMultimodalChunk,
    document_ir: &DocumentIr,
) -> bool {
    if chunk.chunk_type != "page_raster" {
        return false;
    }
    if env_flag_enabled("INGESTION_PAGE_RASTER_WITH_OCR", false) {
        return false;
    }
    document_paddle_ocr_succeeded(document_ir)
}
