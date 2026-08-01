use ingestion::parser::{ExternalParseKind, LocalParseKind};
use ingestion::{DocumentIr, IngestionError};
use uuid::Uuid;

use crate::pdf;

// markitdown 唯一文档解析器（2026-07-31 用户拍板）：文档全类经 markitdown 子进程
// 产出 markdown + Heading/Paragraph IR；standalone 图片走 PaddleOCR。

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
