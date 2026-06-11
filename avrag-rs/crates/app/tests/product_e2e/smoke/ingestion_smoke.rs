//! P0-1: Upload document and verify ingestion completes.

use std::time::Duration;

use crate::product_e2e::{DocumentStatus, TestContext};

#[tokio::test]
async fn upload_document_completes_ingestion() {
    let mut ctx = TestContext::new_smoke().await;

    // 1. Upload document
    let upload = ctx.upload_document("antifragile.txt").await.unwrap();
    // Production `create_document_upload_handler` returns 201 CREATED.
    // If this assertion ever fires, either the API contract changed
    // (intentional) or the helper is no longer threading through the
    // real status code (regression).
    assert_eq!(
        upload.status, 201,
        "expected HTTP 201 from POST .../documents"
    );

    // 2. Wait for ingestion
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(60))
        .await
        .unwrap();
    assert_eq!(status, DocumentStatus::Completed);

    // 3. Verify PG recorded chunks for the ingested document
    let chunk_count = ctx
        .query_document_chunk_count(&upload.document_id)
        .await
        .unwrap();
    assert!(
        chunk_count > 0,
        "expected chunk_count > 0 after successful ingestion, got {chunk_count}"
    );
}
