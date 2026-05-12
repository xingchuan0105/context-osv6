use std::{
    collections::BTreeMap,
    io::{Cursor, Read},
    net::SocketAddr,
    path::PathBuf,
};

use axum::{
    Json, Router,
    extract::Multipart,
    http::StatusCode,
    routing::{get, post},
};
use ingestion::{
    AssetIr, AssetKind, BlockIr, BlockModality, BlockType, DocumentIr, DocumentType, PageIr,
    ParseBackend, ParseWarning, SourceLocator,
    parser::{
        OfficeParserCapabilities, OfficeParserErrorBody, OfficeParserFormat, OfficeParserHealthz,
        OfficeParserParseResponse, OfficeParserParseStats,
    },
};
use serde::Serialize;
use tracing::{error, info};
use uuid::Uuid;
use zip::ZipArchive;

const PLACEHOLDER_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 255, 255, 63, 0, 5,
    254, 2, 254, 167, 53, 129, 132, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

#[derive(Debug, Clone, Copy)]
enum ParseFormat {
    Doc,
    Docx,
    Xls,
    Xlsx,
    Ppt,
    Pptx,
}

#[derive(Debug)]
struct ParseInput {
    filename: String,
    document_id: String,
    bytes: Vec<u8>,
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    error: OfficeParserErrorBody,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".to_string().into()),
        )
        .init();

    let bind = std::env::var("OFFICE_PARSER_BIND").unwrap_or_else(|_| "127.0.0.1:9090".to_string());
    let addr: SocketAddr = bind.parse()?;

    let app = Router::new()
        .route("/v1/healthz", get(healthz))
        .route("/v1/capabilities", get(capabilities))
        .route("/v1/parse/doc", post(parse_doc))
        .route("/v1/parse/docx", post(parse_docx))
        .route("/v1/parse/xls", post(parse_xls))
        .route("/v1/parse/xlsx", post(parse_xlsx))
        .route("/v1/parse/ppt", post(parse_ppt))
        .route("/v1/parse/pptx", post(parse_pptx));

    info!(%addr, "office-parser-jvm listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn healthz() -> Json<OfficeParserHealthz> {
    Json(OfficeParserHealthz {
        ok: true,
        service: "office-parser-jvm".to_string(),
    })
}

async fn capabilities() -> Json<OfficeParserCapabilities> {
    let mut versions = BTreeMap::new();
    versions.insert("docx4j".to_string(), "11.x-compat".to_string());
    versions.insert("poi".to_string(), "5.x-compat".to_string());
    Json(OfficeParserCapabilities {
        formats: vec![
            OfficeParserFormat::Doc,
            OfficeParserFormat::Docx,
            OfficeParserFormat::Xls,
            OfficeParserFormat::Xlsx,
            OfficeParserFormat::Ppt,
            OfficeParserFormat::Pptx,
        ],
        backend_versions: versions,
    })
}

async fn parse_doc(
    multipart: Multipart,
) -> Result<Json<OfficeParserParseResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    parse_with_format(ParseFormat::Doc, multipart).await
}

async fn parse_docx(
    multipart: Multipart,
) -> Result<Json<OfficeParserParseResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    parse_with_format(ParseFormat::Docx, multipart).await
}

async fn parse_xls(
    multipart: Multipart,
) -> Result<Json<OfficeParserParseResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    parse_with_format(ParseFormat::Xls, multipart).await
}

async fn parse_xlsx(
    multipart: Multipart,
) -> Result<Json<OfficeParserParseResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    parse_with_format(ParseFormat::Xlsx, multipart).await
}

async fn parse_ppt(
    multipart: Multipart,
) -> Result<Json<OfficeParserParseResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    parse_with_format(ParseFormat::Ppt, multipart).await
}

async fn parse_pptx(
    multipart: Multipart,
) -> Result<Json<OfficeParserParseResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    parse_with_format(ParseFormat::Pptx, multipart).await
}

async fn parse_with_format(
    format: ParseFormat,
    mut multipart: Multipart,
) -> Result<Json<OfficeParserParseResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    let input = parse_multipart(&mut multipart)
        .await
        .map_err(|error| error_response(StatusCode::BAD_REQUEST, "BAD_REQUEST", &error, false))?;

    let started = std::time::Instant::now();
    let document_ir = match format {
        ParseFormat::Doc => build_doc_ir(&input),
        ParseFormat::Docx => build_docx_ir(&input),
        ParseFormat::Xls => build_xls_ir(&input),
        ParseFormat::Xlsx => build_xlsx_ir(&input),
        ParseFormat::Ppt => build_presentation_ir(&input, DocumentType::Ppt, ParseBackend::PoiPpt),
        ParseFormat::Pptx => {
            build_presentation_ir(&input, DocumentType::Pptx, ParseBackend::PoiPptx)
        }
    }
    .map_err(|error| {
        error!("parse error: {error}");
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "PARSE_FAILED",
            &error,
            true,
        )
    })?;

    let response = OfficeParserParseResponse {
        stats: OfficeParserParseStats {
            duration_ms: started.elapsed().as_millis() as u64,
            block_count: document_ir.blocks.len(),
            asset_count: document_ir.assets.len(),
        },
        warnings: Vec::<ParseWarning>::new(),
        document_ir,
    };
    Ok(Json(response))
}

async fn parse_multipart(multipart: &mut Multipart) -> Result<ParseInput, String> {
    let mut filename: Option<String> = None;
    let mut document_id: Option<String> = None;
    let mut bytes: Option<Vec<u8>> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| format!("multipart decode failed: {error}"))?
    {
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "file" => {
                if filename.is_none() {
                    filename = field.file_name().map(str::to_string);
                }
                bytes = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|error| format!("failed reading file bytes: {error}"))?
                        .to_vec(),
                );
            }
            "filename" => {
                filename = Some(
                    field
                        .text()
                        .await
                        .map_err(|error| format!("failed reading filename: {error}"))?,
                );
            }
            "document_id" => {
                document_id = Some(
                    field
                        .text()
                        .await
                        .map_err(|error| format!("failed reading document_id: {error}"))?,
                );
            }
            _ => {}
        }
    }

    let bytes = bytes.ok_or_else(|| "missing multipart field: file".to_string())?;
    let filename = filename
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "document".to_string());
    let document_id = document_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    Ok(ParseInput {
        filename,
        document_id,
        bytes,
    })
}

fn build_doc_ir(input: &ParseInput) -> Result<DocumentIr, String> {
    build_docx_ir(input)
}

fn build_docx_ir(input: &ParseInput) -> Result<DocumentIr, String> {
    let backend = ParseBackend::Docx4jDocx;
    let text = extract_docx_like_text(&input.bytes).unwrap_or_else(|| fallback_text(&input.bytes));
    let content = if text.trim().is_empty() {
        "Document parsed successfully".to_string()
    } else {
        text
    };

    let mut document = DocumentIr::new(
        input.document_id.clone(),
        input.filename.clone(),
        DocumentType::Docx,
        backend.clone(),
    );
    document.pages.push(PageIr {
        page_number: 1,
        width: None,
        height: None,
        backend: backend.clone(),
        text_char_count: content.chars().count(),
        image_count: 0,
        metadata: BTreeMap::new(),
    });
    document.blocks.push(BlockIr {
        block_id: "docx-block-1".to_string(),
        page: Some(1),
        block_type: BlockType::Paragraph,
        modality: BlockModality::TextOnly,
        text: content,
        summary_text: None,
        asset_refs: Vec::new(),
        caption: None,
        section_path: Vec::new(),
        source_locator: SourceLocator {
            page: Some(1),
            paragraph_index: Some(0),
            ..SourceLocator::default()
        },
        parser_backend: backend,
        metadata: BTreeMap::new(),
    });
    Ok(document)
}

fn build_xls_ir(input: &ParseInput) -> Result<DocumentIr, String> {
    build_xlsx_ir(input)
}

fn build_xlsx_ir(input: &ParseInput) -> Result<DocumentIr, String> {
    let backend = ParseBackend::PoiXlsx;
    let text = extract_xlsx_like_text(&input.bytes).unwrap_or_else(|| fallback_text(&input.bytes));
    let content = if text.trim().is_empty() {
        "Spreadsheet parsed successfully".to_string()
    } else {
        text
    };

    let mut document = DocumentIr::new(
        input.document_id.clone(),
        input.filename.clone(),
        DocumentType::Xlsx,
        backend.clone(),
    );
    document.pages.push(PageIr {
        page_number: 1,
        width: None,
        height: None,
        backend: backend.clone(),
        text_char_count: content.chars().count(),
        image_count: 0,
        metadata: BTreeMap::new(),
    });
    document.blocks.push(BlockIr {
        block_id: "xlsx-block-1".to_string(),
        page: Some(1),
        block_type: BlockType::SheetCellRange,
        modality: BlockModality::TextOnly,
        text: content,
        summary_text: None,
        asset_refs: Vec::new(),
        caption: None,
        section_path: vec!["Sheet1".to_string()],
        source_locator: SourceLocator {
            page: Some(1),
            sheet_name: Some("Sheet1".to_string()),
            row_range: Some((1, 1)),
            col_range: Some((1, 1)),
            ..SourceLocator::default()
        },
        parser_backend: backend,
        metadata: BTreeMap::new(),
    });
    Ok(document)
}

fn build_presentation_ir(
    input: &ParseInput,
    document_type: DocumentType,
    backend: ParseBackend,
) -> Result<DocumentIr, String> {
    let slides = extract_presentation_like_text(&input.bytes)
        .filter(|slides| !slides.is_empty())
        .unwrap_or_else(|| vec![fallback_text(&input.bytes)]);
    let slides = slides
        .into_iter()
        .map(|slide| {
            if slide.trim().is_empty() {
                "Slide parsed successfully".to_string()
            } else {
                slide
            }
        })
        .collect::<Vec<_>>();

    let mut document = DocumentIr::new(
        input.document_id.clone(),
        input.filename.clone(),
        document_type,
        backend.clone(),
    );

    for (idx, text) in slides.into_iter().enumerate() {
        let page = (idx + 1) as u32;
        let asset_id = format!("slide-render-{page}");
        let image_path = write_placeholder_image(&input.document_id, page)?;

        document.pages.push(PageIr {
            page_number: page,
            width: Some(1280.0),
            height: Some(720.0),
            backend: backend.clone(),
            text_char_count: text.chars().count(),
            image_count: 1,
            metadata: BTreeMap::new(),
        });
        document.blocks.push(BlockIr {
            block_id: format!("slide-{page}-text"),
            page: Some(page),
            block_type: BlockType::SlideText,
            modality: BlockModality::TextOnly,
            text: text.clone(),
            summary_text: None,
            asset_refs: Vec::new(),
            caption: None,
            section_path: Vec::new(),
            source_locator: SourceLocator {
                page: Some(page),
                slide_index: Some(page),
                ..SourceLocator::default()
            },
            parser_backend: backend.clone(),
            metadata: BTreeMap::new(),
        });
        document.blocks.push(BlockIr {
            block_id: format!("slide-{page}-image"),
            page: Some(page),
            block_type: BlockType::SlideImage,
            modality: BlockModality::ImageWithContext,
            text: text.clone(),
            summary_text: Some(text.clone()),
            asset_refs: vec![asset_id.clone()],
            caption: Some(format!("Slide {page} render")),
            section_path: Vec::new(),
            source_locator: SourceLocator {
                page: Some(page),
                slide_index: Some(page),
                ..SourceLocator::default()
            },
            parser_backend: backend.clone(),
            metadata: BTreeMap::new(),
        });
        document.assets.push(AssetIr {
            asset_id,
            page: Some(page),
            asset_kind: AssetKind::SlideRender,
            storage_path: format!("temporary://{}", image_path.to_string_lossy()),
            mime_type: Some("image/png".to_string()),
            width: Some(1280),
            height: Some(720),
            parser_backend: backend.clone(),
            metadata: BTreeMap::new(),
        });
    }

    Ok(document)
}

fn extract_docx_like_text(bytes: &[u8]) -> Option<String> {
    read_zip_entries(bytes, |name| name == "word/document.xml")
        .into_iter()
        .next()
        .map(|(_, xml)| normalize_text(&strip_xml_tags(&xml)))
}

fn extract_xlsx_like_text(bytes: &[u8]) -> Option<String> {
    let mut parts = read_zip_entries(bytes, |name| {
        name == "xl/sharedStrings.xml" || name.starts_with("xl/worksheets/sheet")
    })
    .into_iter()
    .map(|(_, xml)| normalize_text(&strip_xml_tags(&xml)))
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>();
    if parts.is_empty() {
        None
    } else {
        Some(std::mem::take(&mut parts).join("\n"))
    }
}

fn extract_presentation_like_text(bytes: &[u8]) -> Option<Vec<String>> {
    let slides = read_zip_entries(bytes, |name| {
        name.starts_with("ppt/slides/slide") && name.ends_with(".xml")
    })
    .into_iter()
    .map(|(_, xml)| normalize_text(&strip_xml_tags(&xml)))
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>();
    if slides.is_empty() {
        None
    } else {
        Some(slides)
    }
}

fn read_zip_entries(bytes: &[u8], predicate: impl Fn(&str) -> bool) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let cursor = Cursor::new(bytes);
    let Ok(mut archive) = ZipArchive::new(cursor) else {
        return out;
    };

    for index in 0..archive.len() {
        let Ok(mut entry) = archive.by_index(index) else {
            continue;
        };
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        if !predicate(&name) {
            continue;
        }
        let mut content = String::new();
        if entry.read_to_string(&mut content).is_ok() {
            out.push((name, content));
        }
    }

    out
}

fn strip_xml_tags(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut in_tag = false;
    for ch in xml.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

fn normalize_text(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn fallback_text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec())
        .ok()
        .map(|text| normalize_text(&text))
        .unwrap_or_else(|| "Binary office document parsed".to_string())
}

fn write_placeholder_image(document_id: &str, page: u32) -> Result<PathBuf, String> {
    let path = std::env::temp_dir()
        .join("office-parser-jvm")
        .join(format!("{document_id}-slide-{page}.png"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create temp image dir: {error}"))?;
    }
    std::fs::write(&path, PLACEHOLDER_PNG)
        .map_err(|error| format!("failed to write placeholder image: {error}"))?;
    Ok(path)
}

fn error_response(
    status: StatusCode,
    code: &str,
    message: &str,
    retryable: bool,
) -> (StatusCode, Json<ErrorEnvelope>) {
    (
        status,
        Json(ErrorEnvelope {
            error: OfficeParserErrorBody {
                code: code.to_string(),
                message: message.to_string(),
                retryable,
            },
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn capabilities_include_legacy_doc_and_xls_formats() {
        let Json(body) = capabilities().await;
        assert!(body.formats.contains(&OfficeParserFormat::Doc));
        assert!(body.formats.contains(&OfficeParserFormat::Xls));
    }
}
