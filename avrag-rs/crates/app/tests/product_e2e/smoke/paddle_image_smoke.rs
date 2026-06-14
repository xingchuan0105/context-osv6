//! PR smoke: standalone PNG through mock Paddle Jobs → indexed chunks (M4 full path).

use std::time::Duration;

use crate::product_e2e::mock_servers::MOCK_PADDLE_IMAGE_OCR_TEXT;
use crate::product_e2e::setup;
use crate::product_e2e::{DocumentStatus, TestContext};

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
async fn paddle_ocr_image_ingest_smoke() {
    super::require_smoke_suite();
    apply_paddle_image_profile();
    assert!(
        paddle_contract_png_path().is_file(),
        "missing bundled fixture paddle-contract.png"
    );

    let mut ctx = TestContext::new_smoke_with_rag().await;
    let path = paddle_contract_png_path().to_string_lossy().to_string();
    let upload = ctx
        .upload_file_from_path(&path)
        .await
        .expect("upload png");
    assert_eq!(upload.status, 201);

    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(180))
        .await
        .expect("wait for ingestion");
    assert_eq!(status, DocumentStatus::Completed);

    let summary = ctx
        .query_latest_backend_summary(&upload.document_id)
        .await
        .expect("backend_summary");
    assert_eq!(
        summary.get("route").and_then(|v| v.as_str()),
        Some("paddle_ocr_image")
    );

    let text_chunks = ctx
        .query_document_chunk_count(&upload.document_id)
        .await
        .expect("text chunk count");
    let figure_mm = ctx
        .query_multimodal_figure_chunk_count(&upload.document_id)
        .await
        .expect("figure multimodal count");
    assert!(
        text_chunks > 0 || figure_mm > 0,
        "expected searchable text or figure chunk, got text={text_chunks} figure_mm={figure_mm}"
    );

    if text_chunks > 0 {
        let pool = sqlx::PgPool::connect(&ctx.pg_url).await.expect("pg pool");
        let doc_id = uuid::Uuid::parse_str(&upload.document_id).expect("doc uuid");
        let row: (String,) = sqlx::query_as(
            "SELECT content FROM chunks WHERE document_id = $1 ORDER BY created_at LIMIT 1",
        )
        .bind(doc_id)
        .fetch_one(&pool)
        .await
        .expect("first chunk content");
        assert!(
            row.0.contains(MOCK_PADDLE_IMAGE_OCR_TEXT),
            "text chunk should contain mock Paddle OCR output"
        );
    }

    if let Some(jobs) = &ctx.mock_paddle_jobs_submitted {
        assert_eq!(
            jobs.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "mock Paddle server should receive exactly one job for standalone image"
        );
    }
}
