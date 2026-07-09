//! Mock Office Parser JVM HTTP server for Product E2E ingest tests.

use super::persistent_runtime::{bind_persistent_listener, spawn_persistent};
use axum::{
    Json, Router,
    extract::Multipart,
    response::IntoResponse,
    routing::{get, post},
};

/// Fixed cell text returned by the mock Office Parser xlsx endpoint.
pub const MOCK_OFFICE_XLSX_TEXT: &str = "Revenue Q1 42";

/// Fixed paragraph text returned by the mock Office Parser docx endpoint.
pub const MOCK_OFFICE_DOCX_TEXT: &str = "Phase0 mini docx ingest probe";

/// Fixed slide text returned by the mock Office Parser pptx endpoint.
pub const MOCK_OFFICE_PPTX_TEXT: &str = "Phase0 mini pptx ingest probe";

fn mock_office_xlsx_document_ir(document_id: &str) -> ingestion::ir::DocumentIr {
    use ingestion::ir::{
        BlockIr, BlockModality, BlockType, DocumentIr, DocumentType, PageIr, ParseBackend,
        SourceLocator,
    };
    DocumentIr {
        document_id: document_id.to_string(),
        title: "contract-xlsx.xlsx".to_string(),
        doc_type: DocumentType::Xlsx,
        primary_backend: ParseBackend::PoiXlsx,
        backend_version: Some("mock-office-parser".to_string()),
        language: Some("en".to_string()),
        metadata: Default::default(),
        pages: vec![PageIr {
            page_number: 1,
            text_char_count: MOCK_OFFICE_XLSX_TEXT.len(),
            image_count: 0,
            ..Default::default()
        }],
        blocks: vec![BlockIr {
            block_id: "sheet-a1".to_string(),
            page: Some(1),
            block_type: BlockType::SheetCellRange,
            modality: BlockModality::TextOnly,
            text: MOCK_OFFICE_XLSX_TEXT.to_string(),
            alt_text: None,
            asset_refs: vec![],
            caption: None,
            section_path: vec![],
            source_locator: SourceLocator {
                page: Some(1),
                ..Default::default()
            },
            parser_backend: ParseBackend::PoiXlsx,
            metadata: Default::default(),
        }],
        assets: vec![],
        warnings: vec![],
    }
}

async fn mock_office_parse_xlsx(mut multipart: Multipart) -> axum::response::Response {
    let mut document_id = "mock-doc".to_string();
    while let Ok(Some(part)) = multipart.next_field().await {
        match part.name().unwrap_or_default() {
            "document_id" => {
                if let Ok(text) = part.text().await {
                    if !text.trim().is_empty() {
                        document_id = text;
                    }
                }
            }
            _ => {}
        }
    }
    let body = ingestion::parser::OfficeParserParseResponse {
        document_ir: mock_office_xlsx_document_ir(&document_id),
        warnings: vec![],
        stats: ingestion::parser::OfficeParserParseStats {
            duration_ms: 1,
            block_count: 1,
            asset_count: 0,
        },
    };
    Json(body).into_response()
}

fn mock_office_docx_document_ir(document_id: &str) -> ingestion::ir::DocumentIr {
    use ingestion::ir::{
        BlockIr, BlockModality, BlockType, DocumentIr, DocumentType, PageIr, ParseBackend,
        SourceLocator,
    };
    DocumentIr {
        document_id: document_id.to_string(),
        title: "phase0-mini.docx".to_string(),
        doc_type: DocumentType::Docx,
        primary_backend: ParseBackend::Docx4jDocx,
        backend_version: Some("mock-office-parser".to_string()),
        language: Some("en".to_string()),
        metadata: Default::default(),
        pages: vec![PageIr {
            page_number: 1,
            text_char_count: MOCK_OFFICE_DOCX_TEXT.len(),
            image_count: 0,
            ..Default::default()
        }],
        blocks: vec![BlockIr {
            block_id: "docx-block-1".to_string(),
            page: Some(1),
            block_type: BlockType::Paragraph,
            modality: BlockModality::TextOnly,
            text: MOCK_OFFICE_DOCX_TEXT.to_string(),
            alt_text: None,
            asset_refs: vec![],
            caption: None,
            section_path: vec![],
            source_locator: SourceLocator {
                page: Some(1),
                ..Default::default()
            },
            parser_backend: ParseBackend::Docx4jDocx,
            metadata: Default::default(),
        }],
        assets: vec![],
        warnings: vec![],
    }
}

async fn mock_office_parse_docx(mut multipart: Multipart) -> axum::response::Response {
    let mut document_id = "mock-doc".to_string();
    while let Ok(Some(part)) = multipart.next_field().await {
        match part.name().unwrap_or_default() {
            "document_id" => {
                if let Ok(text) = part.text().await {
                    if !text.trim().is_empty() {
                        document_id = text;
                    }
                }
            }
            _ => {}
        }
    }
    let body = ingestion::parser::OfficeParserParseResponse {
        document_ir: mock_office_docx_document_ir(&document_id),
        warnings: vec![],
        stats: ingestion::parser::OfficeParserParseStats {
            duration_ms: 1,
            block_count: 1,
            asset_count: 0,
        },
    };
    Json(body).into_response()
}

fn mock_office_pptx_document_ir(document_id: &str) -> ingestion::ir::DocumentIr {
    use ingestion::ir::{
        AssetIr, AssetKind, BlockIr, BlockModality, BlockType, DocumentIr, DocumentType, PageIr,
        ParseBackend, SourceLocator,
    };
    DocumentIr {
        document_id: document_id.to_string(),
        title: "phase0-mini.pptx".to_string(),
        doc_type: DocumentType::Pptx,
        primary_backend: ParseBackend::PoiPptx,
        backend_version: Some("mock-office-parser".to_string()),
        language: Some("en".to_string()),
        metadata: Default::default(),
        pages: vec![PageIr {
            page_number: 1,
            text_char_count: MOCK_OFFICE_PPTX_TEXT.len(),
            image_count: 1,
            backend: ParseBackend::PoiPptx,
            ..Default::default()
        }],
        blocks: vec![
            BlockIr {
                block_id: "slide-1-text".to_string(),
                page: Some(1),
                block_type: BlockType::SlideText,
                modality: BlockModality::TextOnly,
                text: MOCK_OFFICE_PPTX_TEXT.to_string(),
                alt_text: None,
                asset_refs: vec![],
                caption: None,
                section_path: vec![],
                source_locator: SourceLocator {
                    page: Some(1),
                    slide_index: Some(1),
                    ..Default::default()
                },
                parser_backend: ParseBackend::PoiPptx,
                metadata: Default::default(),
            },
            BlockIr {
                block_id: "slide-1-image".to_string(),
                page: Some(1),
                block_type: BlockType::SlideImage,
                modality: BlockModality::ImageWithContext,
                text: "Phase0 mini slide".to_string(),
                alt_text: Some("Phase0 mini slide render".to_string()),
                asset_refs: vec!["slide-render-1".to_string()],
                caption: Some("Phase0 mini slide".to_string()),
                section_path: vec![],
                source_locator: SourceLocator {
                    page: Some(1),
                    slide_index: Some(1),
                    ..Default::default()
                },
                parser_backend: ParseBackend::PoiPptx,
                metadata: Default::default(),
            },
        ],
        assets: vec![AssetIr {
            asset_id: "slide-render-1".to_string(),
            page: Some(1),
            asset_kind: AssetKind::SlideRender,
            storage_path: "mock-assets/slide-1.png".to_string(),
            mime_type: Some("image/png".to_string()),
            width: Some(1280),
            height: Some(720),
            parser_backend: ParseBackend::PoiPptx,
            metadata: Default::default(),
        }],
        warnings: vec![],
    }
}

async fn mock_office_parse_pptx(mut multipart: Multipart) -> axum::response::Response {
    let mut document_id = "mock-doc".to_string();
    while let Ok(Some(part)) = multipart.next_field().await {
        match part.name().unwrap_or_default() {
            "document_id" => {
                if let Ok(text) = part.text().await {
                    if !text.trim().is_empty() {
                        document_id = text;
                    }
                }
            }
            _ => {}
        }
    }
    let body = ingestion::parser::OfficeParserParseResponse {
        document_ir: mock_office_pptx_document_ir(&document_id),
        warnings: vec![],
        stats: ingestion::parser::OfficeParserParseStats {
            duration_ms: 1,
            block_count: 2,
            asset_count: 1,
        },
    };
    Json(body).into_response()
}

async fn mock_office_healthz() -> axum::response::Response {
    Json(ingestion::parser::OfficeParserHealthz {
        ok: true,
        service: "mock-office-parser".to_string(),
    })
    .into_response()
}

/// Start a mock Office Parser HTTP server (`/v1/parse/{docx,pptx,xlsx}`, `/v1/healthz`).
pub async fn start_mock_office_parser_server() -> (String, tokio::sync::oneshot::Sender<()>) {
    let (listener, base_url) = bind_persistent_listener().await;
    let app = Router::new()
        .route("/v1/healthz", get(mock_office_healthz))
        .route("/v1/parse/docx", post(mock_office_parse_docx))
        .route("/v1/parse/pptx", post(mock_office_parse_pptx))
        .route("/v1/parse/xlsx", post(mock_office_parse_xlsx));
    let (abort_tx, abort_rx) = tokio::sync::oneshot::channel::<()>();
    spawn_persistent(async move {
        let server = axum::serve(listener, app);
        tokio::select! {
            _ = server => {},
            _ = abort_rx => {},
        }
    });
    (base_url, abort_tx)
}
