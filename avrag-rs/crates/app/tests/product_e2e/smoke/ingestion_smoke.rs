//! P0-1: Upload document and verify ingestion completes.

use std::time::Duration;

use crate::product_e2e::{DocumentStatus, TestContext};

#[tokio::test]
async fn upload_document_completes_ingestion() {
    let ctx = TestContext::new_smoke().await;

    // 1. Upload document
    let upload = ctx.upload_document("antifragile.txt").await.unwrap();
    assert_eq!(upload.status, 202);

    // 2. Wait for ingestion
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(60))
        .await
        .unwrap();
    assert_eq!(status, DocumentStatus::Completed);

    // 3. Verify PG has summary + TOC + chunks
    // TODO(Phase 1): query PG for document metadata
    // assert!(pg_has_summary(&upload.document_id).await);
    // assert!(pg_has_toc(&upload.document_id).await);
    // assert!(pg_has_chunks(&upload.document_id).await);
}
