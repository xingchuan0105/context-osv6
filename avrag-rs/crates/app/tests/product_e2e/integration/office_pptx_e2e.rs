//! Office PPTX ingest via mock Office Parser → worker → indexed chunks.

use std::time::Duration;

use crate::product_e2e::mock_servers::MOCK_OFFICE_PPTX_TEXT;
use crate::product_e2e::setup;
use crate::product_e2e::{DocumentStatus, TestContext};

fn phase0_mini_pptx_path() -> std::path::PathBuf {
    setup::fixture_path("phase0-mini.pptx").expect("phase0-mini.pptx fixture")
}

#[tokio::test]
async fn office_pptx_ingest_e2e() {
    super::require_integration_suite();
    assert!(
        phase0_mini_pptx_path().is_file(),
        "missing bundled fixture phase0-mini.pptx"
    );

    let mut ctx = TestContext::new_smoke_with_rag().await;
    let path = phase0_mini_pptx_path().to_string_lossy().to_string();
    let upload = ctx.upload_file_from_path(&path).await.expect("upload pptx");
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
    let summary_text = summary.to_string();
    assert!(
        summary_text.contains("office") || summary_text.contains("pptx"),
        "expected office/pptx routing in backend_summary: {summary_text}"
    );

    let chunk_count = ctx
        .query_document_chunk_count(&upload.document_id)
        .await
        .expect("chunk count");
    assert!(chunk_count > 0, "expected indexed chunks after pptx ingest");

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
        row.0.contains(MOCK_OFFICE_PPTX_TEXT),
        "chunk should contain mock office parser pptx text"
    );
}
