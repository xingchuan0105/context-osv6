//! P2-9: Worker processing timeout — document does not hang indefinitely.
//!
//! Product gap: the worker currently has no explicit per-task processing timeout.
//! Tasks rely on `max_attempts` + stale-task recovery (30 min) to eventually
//! dead-letter.  This test is skeleton-only until a worker timeout knob is added.

use std::time::Duration;

use crate::product_e2e::{DocumentStatus, TestContext};

#[tokio::test]
#[ignore = "requires worker per-task timeout mechanism (not yet implemented)"]
async fn worker_processing_timeout_marks_document_failed() {
    let ctx = TestContext::new_smoke_with_rag().await;

    // 1. Upload a document.
    let upload = ctx.upload_document("antifragile.txt").await.unwrap();

    // 2. Stop the worker so the task can never complete.
    //    (In a real timeout test we would instead lower the worker timeout
    //     threshold and let the worker run.)
    //    For now this step is left as a placeholder.

    // 3. Expect the document to eventually reach Failed (or Timeout) status
    //    rather than staying Queued/Processing forever.
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(
        status,
        DocumentStatus::Failed,
        "document should fail or timeout, not hang indefinitely"
    );
}
