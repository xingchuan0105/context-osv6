use ingestion::parser::{
    CodeParser, DocumentParser, ExternalParseKind, HtmlParser, LocalParseKind, OfficeDocType,
    TextParser,
};
use ingestion::{DocumentIr, DocumentType, IngestionError, ParseBackend};
use uuid::Uuid;

use crate::pdf;

use super::processor::PgTaskProcessor;

// PDF page routing (`collect_page_routes`) lives in `crate::pdf::parse` — route stage
// for the probe → route → execute → merge pipeline.

pub(crate) async fn execute_local_parse(
    bytes: &[u8],
    filename: &str,
    document_id: Uuid,
    kind: &LocalParseKind,
) -> Result<DocumentIr, IngestionError> {
    let (doc_type, backend, parser): (DocumentType, ParseBackend, Box<dyn DocumentParser>) =
        match kind {
            LocalParseKind::Text => (
                DocumentType::Text,
                ParseBackend::TextLocal,
                Box::new(TextParser),
            ),
            LocalParseKind::Html => (
                DocumentType::Html,
                ParseBackend::HtmlLocal,
                Box::new(HtmlParser),
            ),
            LocalParseKind::Code => (
                DocumentType::Code,
                ParseBackend::CodeLocal,
                Box::new(CodeParser),
            ),
        };

    let parsed = parser.parse(bytes, filename).await.map_err(|error| {
        IngestionError::parse_local(format!("local parse failed for {filename}: {error}"))
    })?;
    Ok(pdf::document_ir_from_parsed_document(
        document_id,
        filename,
        doc_type,
        backend,
        parsed,
    ))
}

pub(crate) async fn execute_external_parse(
    _processor: &PgTaskProcessor,
    bytes: &[u8],
    filename: &str,
    _object_path: &str,
    document_id: Uuid,
    _parse_run_id: Uuid,
    kind: &ExternalParseKind,
) -> Result<DocumentIr, IngestionError> {
    match kind {
        ExternalParseKind::PaddleOcrImage => {
            pdf::execute_paddle_ocr_image(bytes, filename, document_id).await
        }
    }
}

pub(crate) async fn execute_office_parse(
    processor: &PgTaskProcessor,
    bytes: &[u8],
    filename: &str,
    document_id: Uuid,
    doc_type: &OfficeDocType,
) -> Result<DocumentIr, IngestionError> {
    let client = processor.office_parser_client.as_ref().ok_or_else(|| {
        IngestionError::parse_office(format!(
            "office parse selected for {filename}, but OFFICE_PARSER_BASE_URL is not configured"
        ))
    })?;

    let response = match doc_type {
        OfficeDocType::Docx => {
            client
                .parse_docx(bytes, filename, &document_id.to_string())
                .await
        }
        OfficeDocType::Doc => {
            client
                .parse_doc(bytes, filename, &document_id.to_string())
                .await
        }
        OfficeDocType::Xlsx => {
            client
                .parse_xlsx(bytes, filename, &document_id.to_string())
                .await
        }
        OfficeDocType::Xls => {
            client
                .parse_xls(bytes, filename, &document_id.to_string())
                .await
        }
        OfficeDocType::Pptx => {
            client
                .parse_pptx(bytes, filename, &document_id.to_string())
                .await
        }
        OfficeDocType::Ppt => {
            client
                .parse_ppt(bytes, filename, &document_id.to_string())
                .await
        }
    }
    .map_err(|error| {
        IngestionError::parse_office(format!("office parse failed for {filename}: {error}"))
    })?;

    let mut document_ir = response.document_ir;
    document_ir.document_id = document_id.to_string();
    if document_ir.title.trim().is_empty() {
        document_ir.title = filename.to_string();
    }
    document_ir.warnings.extend(response.warnings);
    Ok(document_ir)
}
