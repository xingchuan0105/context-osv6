//! PaddleOcrImage routing metadata beyond PR smoke (`smoke::paddle_image_smoke` covers full ingest path).

use std::time::Duration;

use crate::product_e2e::setup;
use crate::product_e2e::{
    DocumentStatus, TestContext, assertions::assert_no_mineru_in_backend_summary,
};

fn paddle_contract_png_path() -> std::path::PathBuf {
    setup::fixture_path("paddle-contract.png").expect("paddle-contract.png fixture")
}

fn apply_paddle_image_profile() {
    unsafe {
        std::env::set_var("INGESTION_TRIPLET_ENABLED", "0");
        std::env::set_var("INGESTION_VLM_TRIPLET_ENABLED", "0");
        std::env::set_var("INGESTION_VLM_SUMMARY_ENABLED", "0");
        std::env::set_var("INGESTION_PAGE_RASTER_WITH_OCR", "0");
    }
}

#[tokio::test]
async fn paddle_ocr_image_routing_metadata_contract() {
    super::require_integration_suite();
    apply_paddle_image_profile();
    let path = paddle_contract_png_path();
    assert!(path.is_file(), "missing paddle-contract.png");

    let mut ctx = TestContext::new_smoke_with_rag().await;
    let upload = ctx
        .upload_file_from_path(&path.to_string_lossy())
        .await
        .expect("upload png");
    assert_eq!(upload.status, 201);

    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(180))
        .await
        .expect("wait for ingestion");
    assert_eq!(status, DocumentStatus::Completed);

    let mime_type = ctx
        .query_document_mime_type(&upload.document_id)
        .await
        .expect("document mime_type");
    assert!(
        mime_type.starts_with("image/"),
        "expected image/* mime_type for png upload, got {mime_type}"
    );

    let summary = ctx
        .query_latest_backend_summary(&upload.document_id)
        .await
        .expect("backend_summary");
    let summary_text = summary.to_string();
    assert_eq!(
        summary.get("route").and_then(|v| v.as_str()),
        Some("paddle_ocr_image")
    );
    assert_eq!(
        summary.get("reason").and_then(|v| v.as_str()),
        Some("image_file")
    );

    let ingest_routing = summary
        .get("ingest_routing")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        ingest_routing
            .get("pdf_route_mode")
            .and_then(|v| v.as_str()),
        Some("paddle_image"),
        "expected pdf_route_mode=paddle_image: {summary_text}"
    );
    assert_eq!(
        ingest_routing
            .get("paddle_jobs_count")
            .and_then(|v| v.as_str()),
        Some("1")
    );
    assert_eq!(
        ingest_routing.get("ocr_backend").and_then(|v| v.as_str()),
        Some("paddle_jobs")
    );
    assert_no_mineru_in_backend_summary(&summary);
}
