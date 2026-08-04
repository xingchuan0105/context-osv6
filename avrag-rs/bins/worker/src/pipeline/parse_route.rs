use ingestion::parser::{ExternalParseKind, LocalParseKind};
use ingestion::{DocumentIr, IngestionError};
use uuid::Uuid;

use crate::pdf;

// 按格式分工（2026-08-05）：PDF→liteparse、Office/ODF/RTF/EPUB/CSV→anydoc、
// 文本/代码→markitdown；standalone 图片走 PaddleOCR；扫描版 PDF 转整本 PaddleOCR。
// 见 `docs/plans/2026-08-05-parser-pipeline-anydoc.md`。

pub(crate) async fn execute_local_parse(
    bytes: &[u8],
    filename: &str,
    document_id: Uuid,
    kind: &LocalParseKind,
) -> Result<(DocumentIr, Option<String>), IngestionError> {
    match kind {
        LocalParseKind::Markitdown => {
            let (ir, markdown) =
                ingestion::parser::parse_markitdown_document_ir(document_id, filename, bytes)
                    .await
                    .map_err(|error| {
                        IngestionError::parse(format!(
                            "markitdown parse failed for {filename}: {error}"
                        ))
                    })?;
            Ok((ir, Some(markdown)))
        }
        LocalParseKind::LiteparseV2Pdf => {
            let (ir, markdown) =
                ingestion::parser::parse_liteparse_pdf_document_ir(document_id, filename, bytes)
                    .await
                    .map_err(|error| {
                        IngestionError::parse(format!(
                            "liteparse parse failed for {filename}: {error}"
                        ))
                    })?;
            // 扫描检测：liteparse 提取近空 → 整本转 PaddleOCR，避免 EmptyIndex 死档。
            if ingestion::parser::is_scanned_markdown(&markdown) {
                let ocr_ir = pdf::execute_paddle_ocr_pdf(bytes, filename, document_id)
                    .await
                    .map_err(|error| {
                        IngestionError::parse(format!(
                            "scanned PDF paddle OCR failed for {filename}: {error}"
                        ))
                    })?;
                return Ok((ocr_ir, None));
            }
            Ok((ir, Some(markdown)))
        }
        LocalParseKind::Anydoc => {
            let (ir, markdown) =
                ingestion::parser::parse_anydoc_document_ir(document_id, filename, bytes)
                    .await
                    .map_err(|error| {
                        IngestionError::parse(format!(
                            "anydoc parse failed for {filename}: {error}"
                        ))
                    })?;
            Ok((ir, Some(markdown)))
        }
    }
}

pub(crate) async fn execute_external_parse(
    bytes: &[u8],
    filename: &str,
    document_id: Uuid,
    kind: &ExternalParseKind,
) -> Result<DocumentIr, IngestionError> {
    match kind {
        ExternalParseKind::PaddleOcrImage => {
            pdf::execute_paddle_ocr_image(bytes, filename, document_id).await
        }
    }
}
