//! LiteParse staging E2E: bundled `phase0-mini.pdf` through upload → worker → index.

use std::time::Duration;

use crate::product_e2e::setup;
use crate::product_e2e::{
    DocumentStatus, TestContext,
    assertions::{assert_liteparse_hybrid_backend_summary, assert_no_mineru_in_backend_summary},
};

fn apply_liteparse_staging_profile() {
    unsafe {
        std::env::set_var("INGESTION_PDF_MAX_PAGES", "8");
        std::env::set_var("INGESTION_TRIPLET_ENABLED", "0");
        std::env::set_var("INGESTION_VLM_TRIPLET_ENABLED", "0");
        std::env::set_var("INGESTION_VLM_SUMMARY_ENABLED", "0");
        std::env::set_var("INGESTION_PAGE_RASTER_WITH_OCR", "0");
    }
}

fn phase0_mini_fixture_path() -> std::path::PathBuf {
    setup::fixture_path("phase0-mini.pdf").expect("phase0-mini.pdf fixture")
}

#[tokio::test]
async fn phase0_mini_liteparse_pdf_ingest_e2e() {
    super::require_integration_suite();
    apply_liteparse_staging_profile();
    assert!(
        phase0_mini_fixture_path().is_file(),
        "missing bundled fixture phase0-mini.pdf"
    );

    let mut ctx = TestContext::new_smoke_with_rag().await;
    let path = phase0_mini_fixture_path().to_string_lossy().to_string();
    let upload = ctx.upload_file_from_path(&path).await.expect("upload pdf");
    assert_eq!(upload.status, 201);

    let status = match ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(180))
        .await
    {
        Ok(status) => status,
        Err(error) => {
            let task_debug = ctx
                .query_ingestion_task_debug(&upload.document_id)
                .await
                .unwrap_or_else(|query_error| {
                    serde_json::json!({ "query_error": query_error.to_string() })
                });
            eprintln!(
                "[liteparse_e2e] ingestion task debug: {}",
                task_debug
            );
            eprintln!(
                "[liteparse_e2e] worker log tail:\n{}",
                ctx.worker_log_tail(120)
            );
            panic!("wait for ingestion: {error}");
        }
    };
    assert_eq!(status, DocumentStatus::Completed);

    let chunk_units = ctx
        .query_ingested_chunk_units(&upload.document_id)
        .await
        .expect("chunk units");
    assert!(
        chunk_units > 0,
        "expected indexed chunks after LiteParse ingest, got {chunk_units}"
    );

    let summary = ctx
        .query_latest_backend_summary(&upload.document_id)
        .await
        .expect("backend_summary");
    assert_liteparse_hybrid_backend_summary(&summary);
    assert_no_mineru_in_backend_summary(&summary);
    let summary_text = summary.to_string();

    let ingest_routing = summary
        .get("ingest_routing")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        ingest_routing.get("pdf_route_mode").and_then(|v| v.as_str()),
        Some("liteparse_hybrid"),
        "expected liteparse_hybrid route mode in ingest_routing: {summary_text}"
    );

    if ingest_routing.contains_key("paddle_jobs_count") {
        assert_eq!(
            ingest_routing.get("ocr_backend").and_then(|v| v.as_str()),
            Some("paddle_jobs"),
            "paddle OCR pages should record ocr_backend=paddle_jobs"
        );
    } else if let Some(warnings) = ingest_routing.get("paddle_warnings").and_then(|v| v.as_array())
    {
        assert!(
            warnings.iter().any(|w| {
                w.get("code")
                    .and_then(|c| c.as_str())
                    .is_some_and(|code| {
                        code == "paddle_job_failed" || code == "paddle_job_budget_exhausted"
                    })
            }),
            "when paddle jobs did not run, ingest_routing should record a paddle warning"
        );
    }

    let page_raster_mm = ctx
        .query_multimodal_page_raster_count(&upload.document_id)
        .await
        .expect("page_raster multimodal count");
    assert_eq!(
        page_raster_mm, 0,
        "digital PDF should not produce page_raster multimodal chunks"
    );
}
