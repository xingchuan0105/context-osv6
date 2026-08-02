use std::collections::BTreeMap;

use ingestion::parser::{PaddleOcrClient, PaddleOcrConfig, PaddleOcrPageResult};
use ingestion::{
    AssetIr, BlockIr, BlockModality, BlockType, DocumentIr, DocumentType, IngestionError, PageIr,
    ParseBackend, SourceLocator,
};
use uuid::Uuid;

pub fn build_document_ir_from_paddle(
    document_id: Uuid,
    filename: &str,
    pages: &[PaddleOcrPageResult],
    table_ocr_pages: &std::collections::HashSet<u32>,
) -> DocumentIr {
    let mut ir = DocumentIr::new(
        document_id.to_string(),
        filename.to_string(),
        DocumentType::Pdf,
        ParseBackend::PaddleOcrPdf,
    );
    ir.metadata
        .insert("ocr_backend".to_string(), "paddle_jobs".to_string());

    for page in pages {
        let is_table_page = table_ocr_pages.contains(&page.page_number);
        ir.pages.push(PageIr {
            page_number: page.page_number,
            width: None,
            height: None,
            backend: ParseBackend::PaddleOcrPdf,
            text_char_count: page.text.len(),
            image_count: page.figures.len(),
            metadata: Default::default(),
        });

        if !page.text.is_empty() {
            ir.blocks.push(BlockIr {
                block_id: format!("paddle-p{}-text", page.page_number),
                page: Some(page.page_number),
                block_type: if is_table_page {
                    BlockType::Table
                } else {
                    BlockType::Paragraph
                },
                modality: BlockModality::TextOnly,
                text: page.text.clone(),
                alt_text: None,
                asset_refs: Vec::new(),
                caption: None,
                section_path: Vec::new(),
                source_locator: SourceLocator {
                    page: Some(page.page_number),
                    ..SourceLocator::default()
                },
                parser_backend: ParseBackend::PaddleOcrPdf,
                metadata: Default::default(),
            });
        }

        for (fig_idx, figure) in page.figures.iter().enumerate() {
            let asset_id = format!("paddle-p{}-fig{}", page.page_number, fig_idx);
            let mut asset_metadata = BTreeMap::new();
            asset_metadata.insert("source".to_string(), "paddle_ocr".to_string());
            asset_metadata.insert("ephemeral_url".to_string(), "true".to_string());
            asset_metadata.insert("original_url".to_string(), figure.image_url.clone());
            ir.assets.push(AssetIr {
                asset_id: asset_id.clone(),
                page: Some(page.page_number),
                asset_kind: ingestion::AssetKind::Image,
                storage_path: figure.image_url.clone(),
                mime_type: None,
                width: None,
                height: None,
                parser_backend: ParseBackend::PaddleOcrPdf,
                metadata: asset_metadata,
            });

            ir.blocks.push(BlockIr {
                block_id: format!("paddle-p{}-fig{}", page.page_number, fig_idx),
                page: Some(page.page_number),
                block_type: BlockType::Figure,
                modality: BlockModality::ImageWithContext,
                text: figure.surrounding_text.clone(),
                alt_text: Some(figure.image_key.clone()),
                asset_refs: vec![asset_id],
                caption: None,
                section_path: Vec::new(),
                source_locator: SourceLocator {
                    page: Some(page.page_number),
                    ..SourceLocator::default()
                },
                parser_backend: ParseBackend::PaddleOcrPdf,
                metadata: BTreeMap::from([(
                    "paddle_image_key".to_string(),
                    figure.image_key.clone(),
                )]),
            });
        }
    }

    ir
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
    ir.doc_type = DocumentType::Image;
    ir.metadata.insert(
        "ingest_route_version".to_string(),
        "liteparse-v1".to_string(),
    );
    ir.metadata
        .insert("pdf_route_mode".to_string(), "paddle_image".to_string());
    ir.metadata
        .insert("paddle_jobs_requested".to_string(), "1".to_string());
    ir.metadata
        .insert("paddle_jobs_count".to_string(), "1".to_string());
    ir.metadata
        .insert("paddle_jobs_used".to_string(), "1".to_string());
    Ok(ir)
}

/// Scanned PDF ingest：整本 PDF 一个 Paddle job → 逐页 OCR 文本块。
///
/// 2026-08-02 起由 liteparse 路由触发：`lit parse` 产出近空 markdown（`is_scanned_markdown`）
/// 即切此路径，避免扫描件空 IR → 终端 `EmptyIndex` 死档。整本 PDF 单 job 返回逐页结果
/// （`PaddleOcrClient::ocr_pdf_bytes`），无需页面渲染器。`doc_type` 保持 `Pdf`。
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
    ir.metadata.insert(
        "ingest_route_version".to_string(),
        "liteparse-v1".to_string(),
    );
    ir.metadata
        .insert("pdf_route_mode".to_string(), "paddle_ocr_pdf".to_string());
    ir.metadata
        .insert("paddle_jobs_requested".to_string(), "1".to_string());
    ir.metadata
        .insert("paddle_jobs_count".to_string(), "1".to_string());
    ir.metadata
        .insert("paddle_jobs_used".to_string(), "1".to_string());
    Ok(ir)
}

#[cfg(test)]
mod tests {
    use super::*;
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

    /// Documents metadata contract for standalone image ingest (`execute_paddle_ocr_image`).
    #[test]
    fn paddle_image_route_metadata_contract() {
        let pages = vec![PaddleOcrPageResult {
            page_number: 1,
            text: "image ocr text".to_string(),
            figures: vec![],
        }];
        let mut ir =
            build_document_ir_from_paddle(Uuid::new_v4(), "photo.png", &pages, &HashSet::new());
        ir.doc_type = DocumentType::Image;
        ir.metadata.insert(
            "ingest_route_version".to_string(),
            "liteparse-v1".to_string(),
        );
        ir.metadata
            .insert("pdf_route_mode".to_string(), "paddle_image".to_string());
        ir.metadata
            .insert("paddle_jobs_requested".to_string(), "1".to_string());
        ir.metadata
            .insert("paddle_jobs_count".to_string(), "1".to_string());
        ir.metadata
            .insert("paddle_jobs_used".to_string(), "1".to_string());

        assert_eq!(ir.doc_type, DocumentType::Image);
        assert_eq!(
            ir.metadata.get("pdf_route_mode").map(String::as_str),
            Some("paddle_image")
        );
        assert_eq!(
            ir.metadata.get("paddle_jobs_count").map(String::as_str),
            Some("1")
        );
        assert!(
            !ir.blocks.is_empty(),
            "image OCR should emit searchable text blocks"
        );
    }
}
