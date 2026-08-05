use ingestion::parser::{PaddleOcrClient, PaddleOcrConfig};
use ingestion::{DocumentIr, DocumentType, IngestionError};
use uuid::Uuid;

pub use ingestion::parser::build_document_ir_from_paddle;

/// Apply image-route product metadata after Paddle IR assembly (testable pure).
pub fn apply_paddle_image_route_metadata(ir: &mut DocumentIr) {
    ir.doc_type = DocumentType::Image;
    ir.metadata.insert(
        "ingest_route_version".to_string(),
        "paddle-v1".to_string(),
    );
    ir.metadata
        .insert("pdf_route_mode".to_string(), "paddle_image".to_string());
    ir.metadata
        .insert("paddle_jobs_requested".to_string(), "1".to_string());
    ir.metadata
        .insert("paddle_jobs_count".to_string(), "1".to_string());
    ir.metadata
        .insert("paddle_jobs_used".to_string(), "1".to_string());
}

/// Apply scanned-PDF route product metadata after Paddle IR assembly (testable pure).
pub fn apply_paddle_pdf_route_metadata(ir: &mut DocumentIr) {
    ir.metadata.insert(
        "ingest_route_version".to_string(),
        "paddle-v1".to_string(),
    );
    ir.metadata
        .insert("pdf_route_mode".to_string(), "paddle_ocr_pdf".to_string());
    ir.metadata
        .insert("paddle_jobs_requested".to_string(), "1".to_string());
    ir.metadata
        .insert("paddle_jobs_count".to_string(), "1".to_string());
    ir.metadata
        .insert("paddle_jobs_used".to_string(), "1".to_string());
}

/// Standalone image ingest: 1 file = 1 Paddle Job (page 1).
pub async fn execute_paddle_ocr_image(
    bytes: &[u8],
    filename: &str,
    document_id: Uuid,
) -> Result<DocumentIr, IngestionError> {
    let config = PaddleOcrConfig::from_env()
        .map_err(|e| IngestionError::parse(format!("PaddleOCR config error: {e}")))?;
    let client = PaddleOcrClient::new(config);
    let page_result = client
        .ocr_image_bytes(bytes, filename)
        .await
        .map_err(|e| IngestionError::parse(format!("PaddleOCR image job failed: {e}")))?;

    let table_pages = std::collections::HashSet::new();
    let mut ir = build_document_ir_from_paddle(
        document_id,
        filename,
        std::slice::from_ref(&page_result),
        &table_pages,
    );
    apply_paddle_image_route_metadata(&mut ir);
    Ok(ir)
}

/// Scanned PDF ingest：整本 PDF 一个 Paddle job → 逐页 OCR 文本块。
pub async fn execute_paddle_ocr_pdf(
    bytes: &[u8],
    filename: &str,
    document_id: Uuid,
) -> Result<DocumentIr, IngestionError> {
    let config = PaddleOcrConfig::from_env()
        .map_err(|e| IngestionError::parse(format!("PaddleOCR config error: {e}")))?;
    let client = PaddleOcrClient::new(config);
    let pages = client
        .ocr_pdf_bytes(bytes, 1)
        .await
        .map_err(|e| IngestionError::parse(format!("PaddleOCR PDF job failed: {e}")))?;

    let table_pages = std::collections::HashSet::new();
    let mut ir = build_document_ir_from_paddle(document_id, filename, &pages, &table_pages);
    apply_paddle_pdf_route_metadata(&mut ir);
    Ok(ir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingestion::{BlockType, parser::PaddleOcrPageResult};
    use std::collections::HashSet;

    #[test]
    fn table_ocr_pages_emit_table_blocks() {
        let pages = vec![PaddleOcrPageResult {
            page_number: 2,
            text: "| a | b |".to_string(),
            figures: vec![],
        }];
        let table_pages = HashSet::from([2]);
        let ir = build_document_ir_from_paddle(Uuid::new_v4(), "t.pdf", &pages, &table_pages);
        assert_eq!(ir.blocks.len(), 1);
        assert_eq!(ir.blocks[0].block_type, BlockType::Table);
        assert_eq!(
            ir.metadata.get("ocr_backend").map(String::as_str),
            Some("paddle_jobs")
        );
    }

    #[test]
    fn image_route_metadata_contract() {
        let pages = vec![PaddleOcrPageResult {
            page_number: 1,
            text: "hi".to_string(),
            figures: vec![],
        }];
        let mut ir =
            build_document_ir_from_paddle(Uuid::new_v4(), "photo.png", &pages, &HashSet::new());
        apply_paddle_image_route_metadata(&mut ir);
        assert_eq!(ir.blocks.len(), 1);
        assert_eq!(ir.doc_type, DocumentType::Image);
        assert_eq!(
            ir.metadata.get("ingest_route_version").map(String::as_str),
            Some("paddle-v1")
        );
        assert_eq!(
            ir.metadata.get("pdf_route_mode").map(String::as_str),
            Some("paddle_image")
        );
        assert_eq!(
            ir.metadata.get("paddle_jobs_used").map(String::as_str),
            Some("1")
        );
    }

    #[test]
    fn pdf_route_metadata_contract() {
        let pages = vec![PaddleOcrPageResult {
            page_number: 1,
            text: "scan".to_string(),
            figures: vec![],
        }];
        let mut ir =
            build_document_ir_from_paddle(Uuid::new_v4(), "scan.pdf", &pages, &HashSet::new());
        apply_paddle_pdf_route_metadata(&mut ir);
        assert_eq!(
            ir.metadata.get("pdf_route_mode").map(String::as_str),
            Some("paddle_ocr_pdf")
        );
        assert_eq!(
            ir.metadata.get("ingest_route_version").map(String::as_str),
            Some("paddle-v1")
        );
    }
}
