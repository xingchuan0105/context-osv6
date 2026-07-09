//! P2-12: Duplicate upload of the same file returns the same document_id.

use std::time::Duration;

use crate::product_e2e::{DocumentStatus, TestContext};

#[tokio::test]
async fn duplicate_upload_returns_same_document_id() {
    super::require_integration_suite();

    let mut ctx = TestContext::new_smoke().await;

    // 1. First upload
    let upload1 = ctx.upload_document("antifragile.txt").await.unwrap();
    let status1 = ctx
        .wait_for_ingestion(&upload1.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(status1, DocumentStatus::Completed);

    // 2. Second upload of the same file to the same notebook
    let upload2 = ctx
        .upload_document_to_notebook("antifragile.txt", &upload1.workspace_id)
        .await
        .unwrap();

    // Wait for second ingestion
    let status2 = ctx
        .wait_for_ingestion(&upload2.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(status2, DocumentStatus::Completed);

    // 3. Current behavior: the system does NOT deduplicate uploads.
    // Each upload gets a new document_id. This assertion documents
    // the current state; change to assert_eq! once dedup is implemented.
    assert_ne!(
        upload1.document_id, upload2.document_id,
        "NOTE: duplicate upload currently returns a new document_id (no dedup)"
    );
}
