//! PaddleOcrImage asset contract (M4): standalone PNG through mock Paddle Jobs → indexed chunks.
//!
//! ## Expected behavior (no real Paddle dependency)
//!
//! | Stage | Contract |
//! |-------|----------|
//! | Router | `ParseRoute::PaddleOcrImage` / reason `image_file` |
//! | Worker | `execute_paddle_ocr_image`: 1 file = 1 Paddle Job |
//! | IR | `DocumentType::Image`, `pdf_route_mode=paddle_image`, `paddle_jobs_count=1` |
//! | Index | ≥1 searchable **text** chunk **or** ≥1 **figure** multimodal chunk |
//! | Anti-regression | No MinerU / LiteParse text path for standalone images |
//!
//! Mock Paddle returns OCR text-only (no remote figure URLs) so worker asset mirroring
//! does not require DNS for ephemeral Paddle CDN hosts in CI.

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
async fn paddle_ocr_image_ingest_e2e() {
    super::require_integration_suite();
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
            eprintln!("[paddle_image_e2e] ingestion task debug: {task_debug}");
            eprintln!(
                "[paddle_image_e2e] worker log tail:\n{}",
                ctx.worker_log_tail(120)
            );
            panic!("wait for ingestion: {error}");
        }
    };
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
        Some("paddle_ocr_image"),
        "expected PaddleOcrImage route in backend_summary: {summary_text}"
    );
    assert_eq!(
        summary.get("reason").and_then(|v| v.as_str()),
        Some("image_file"),
        "expected image_file reason in backend_summary: {summary_text}"
    );

    let plan_kind = summary
        .get("plan")
        .and_then(|plan| {
            plan.get("external")
                .or_else(|| plan.get("External"))
                .and_then(|v| v.get("kind"))
                .and_then(|v| v.as_str())
        })
        .unwrap_or_default();
    assert_eq!(
        plan_kind, "paddle_ocr_image",
        "expected external plan kind paddle_ocr_image: {summary_text}"
    );

    let ingest_routing = summary
        .get("ingest_routing")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        ingest_routing.get("pdf_route_mode").and_then(|v| v.as_str()),
        Some("paddle_image"),
        "expected pdf_route_mode=paddle_image in ingest_routing: {summary_text}"
    );
    assert_eq!(
        ingest_routing.get("paddle_jobs_count").and_then(|v| v.as_str()),
        Some("1"),
        "expected paddle_jobs_count=1 in ingest_routing: {summary_text}"
    );
    assert_eq!(
        ingest_routing.get("ocr_backend").and_then(|v| v.as_str()),
        Some("paddle_jobs"),
        "expected ocr_backend=paddle_jobs in ingest_routing: {summary_text}"
    );

    assert!(
        !summary_text.contains("mineru"),
        "standalone image ingest must not reference MinerU: {summary_text}"
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
        "expected searchable text chunk or figure multimodal chunk, got text={text_chunks} figure_mm={figure_mm}"
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
