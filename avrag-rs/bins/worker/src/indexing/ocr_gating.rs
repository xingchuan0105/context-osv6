use ingestion::{DocumentIr, ParseBackend};

pub fn document_paddle_ocr_succeeded(document_ir: &DocumentIr) -> bool {
    if document_ir.metadata.get("ocr_backend").map(|s| s.as_str()) == Some("paddle") {
        return true;
    }
    document_ir
        .pages
        .iter()
        .any(|page| page.backend == ParseBackend::PaddleOcrPdf)
}
