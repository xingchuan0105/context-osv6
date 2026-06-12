use anyhow::Result;
use ingestion::parser::{
    CodeParser, DocumentParser, ExternalParseKind, HtmlParser, LocalParseKind, OfficeDocType,
    TextParser,
};
use ingestion::{DocumentIr, DocumentType, IngestionError, ParseBackend};
use uuid::Uuid;

use crate::pdf;

use super::processor::PgTaskProcessor;

pub(crate) async fn resolve_mineru_source_url(
    processor: &PgTaskProcessor,
    object_path: &str,
) -> Result<Option<String>, IngestionError> {
    if object_path.trim().is_empty() {
        return Ok(None);
    }
    if common::is_remote_url(object_path) {
        return Ok(Some(object_path.to_string()));
    }

    let presigned = processor
        .object_store
        .presigned_get_url(object_path, processor.asset_url_ttl_secs.max(300))
        .await
        .map_err(|error| IngestionError::StateSink(error.to_string()))?;
    Ok(Some(presigned))
}

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
        IngestionError::StateSink(format!("local parse failed for {filename}: {error}"))
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
    processor: &PgTaskProcessor,
    bytes: &[u8],
    filename: &str,
    object_path: &str,
    document_id: Uuid,
    kind: &ExternalParseKind,
) -> Result<DocumentIr, IngestionError> {
    let mineru = processor.mineru_client.as_ref().ok_or_else(|| {
        IngestionError::StateSink(format!(
            "external parse selected for {filename}, but MINERU is not configured"
        ))
    })?;
    let source_url = resolve_mineru_source_url(processor, object_path).await?;

    match kind {
        ExternalParseKind::MineruImage => {
            let normalized = mineru
                .parse(bytes, filename, source_url.as_deref())
                .await
                .map_err(|error| {
                    IngestionError::StateSink(format!(
                        "MinerU precise parse failed for {filename}: {error}"
                    ))
                })?;
            let doc_type = DocumentType::from_filename(filename);
            Ok(DocumentIr::from_normalized_document(
                document_id.to_string(),
                doc_type,
                ParseBackend::MineruImage,
                &normalized,
            ))
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
        IngestionError::StateSink(format!(
            "office parse selected for {filename}, but OFFICE_PARSER_BASE_URL is not configured"
        ))
    })?;

    let response = match doc_type {
        OfficeDocType::Docx => {
            client
                .parse_docx(bytes, filename, &document_id.to_string())
                .await
        }
        OfficeDocType::Xlsx => {
            client
                .parse_xlsx(bytes, filename, &document_id.to_string())
                .await
        }
        OfficeDocType::Ppt => {
            client
                .parse_ppt(bytes, filename, &document_id.to_string())
                .await
        }
        OfficeDocType::Pptx => {
            client
                .parse_pptx(bytes, filename, &document_id.to_string())
                .await
        }
        OfficeDocType::Doc => {
            client
                .parse_doc(bytes, filename, &document_id.to_string())
                .await
        }
        OfficeDocType::Xls => {
            client
                .parse_xls(bytes, filename, &document_id.to_string())
                .await
        }
    }
    .map_err(|error| {
        IngestionError::StateSink(format!("office parse failed for {filename}: {error}"))
    })?;

    let mut document_ir = response.document_ir;
    document_ir.document_id = document_id.to_string();
    if document_ir.title.trim().is_empty() {
        document_ir.title = filename.to_string();
    }
    document_ir.warnings.extend(response.warnings);
    Ok(document_ir)
}
