//! P2-9: Worker processing timeout — document does not hang indefinitely.
//!
//! Test strategy: create a TestContext with worker per-task timeout of 1s,
//! then upload a document whose ingestion pipeline normally takes >1s.
//! The worker should abort the task and transition the document to Failed.

use std::time::Duration;

use crate::product_e2e::{DocumentStatus, TestContext};

#[tokio::test]
async fn worker_processing_timeout_marks_document_failed() {
    let mut ctx = TestContext::new_smoke_with_rag_and_timeout(1).await;

    // 1. Upload a document.
    let upload = ctx.upload_document("antifragile.txt").await.unwrap();

    // Force max_attempts = 1 so the first timeout immediately dead-letters.
    ctx.set_ingestion_max_attempts(&upload.document_id, 1)
        .await
        .expect("set max_attempts");

    // 2. Wait for ingestion — should eventually reach Failed because the
    //    worker timeout (1s) is shorter than the full pipeline (~5-8s).
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(
        status,
        DocumentStatus::Failed,
        "document should fail when worker per-task timeout is exceeded"
    );
}
