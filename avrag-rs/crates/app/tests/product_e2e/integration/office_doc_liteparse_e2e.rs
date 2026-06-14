//! docx → LibreOffice PDF → LiteParse hybrid ingest (P4 office convert path).

use std::time::Duration;

use crate::product_e2e::setup;
use crate::product_e2e::{
    DocumentStatus, TestContext,
    assertions::{assert_liteparse_hybrid_backend_summary, assert_no_mineru_in_backend_summary},
};

fn phase0_mini_docx_path() -> std::path::PathBuf {
    setup::fixture_path("phase0-mini.docx").expect("phase0-mini.docx fixture")
}

fn apply_doc_liteparse_profile() {
    unsafe {
        std::env::set_var("INGESTION_PDF_MAX_PAGES", "8");
        std::env::set_var("INGESTION_TRIPLET_ENABLED", "0");
        std::env::set_var("INGESTION_VLM_TRIPLET_ENABLED", "0");
        std::env::set_var("INGESTION_VLM_SUMMARY_ENABLED", "0");
        std::env::set_var("INGESTION_PAGE_RASTER_WITH_OCR", "0");
    }
}

fn libreoffice_available() -> bool {
    std::process::Command::new("which")
        .arg("libreoffice")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[tokio::test]
#[ignore = "requires libreoffice headless; run locally or in staging image"]
async fn minimal_docx_liteparse_pdf_ingest_e2e() {
    super::require_integration_suite();
    if !libreoffice_available() {
        panic!("libreoffice not on PATH — install LibreOffice or skip this staging test");
    }
    apply_doc_liteparse_profile();
    assert!(
        phase0_mini_docx_path().is_file(),
        "missing bundled fixture phase0-mini.docx"
    );

    let mut ctx = TestContext::new_smoke_with_rag().await;
    let path = phase0_mini_docx_path().to_string_lossy().to_string();
    let upload = ctx.upload_file_from_path(&path).await.expect("upload docx");
    assert_eq!(upload.status, 201);

    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(240))
        .await
        .expect("wait for ingestion");
    assert_eq!(status, DocumentStatus::Completed);

    let chunk_units = ctx
        .query_ingested_chunk_units(&upload.document_id)
        .await
        .expect("chunk units");
    assert!(
        chunk_units > 0,
        "expected indexed chunks after docx→pdf LiteParse ingest, got {chunk_units}"
    );

    let summary = ctx
        .query_latest_backend_summary(&upload.document_id)
        .await
        .expect("backend_summary");
    assert_liteparse_hybrid_backend_summary(&summary);
    assert_no_mineru_in_backend_summary(&summary);
}
