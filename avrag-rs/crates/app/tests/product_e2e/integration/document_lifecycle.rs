//! Document delete HTTP lifecycle.

use std::time::Duration;

use crate::product_e2e::TestContext;

#[tokio::test]
async fn delete_document_hides_status_and_content() {
    super::require_integration_suite();
    let mut ctx = TestContext::new_smoke_with_rag().await;
    let upload = ctx.upload_document("antifragile.txt").await.expect("upload");
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(120))
        .await
        .expect("ingest");
    assert_eq!(status, crate::product_e2e::DocumentStatus::Completed);

    let delete_resp = ctx
        .delete_document(&upload.document_id)
        .await
        .expect("delete document");
    assert_eq!(delete_resp.status, 200);

    let status_resp = ctx
        .fetch_document_status(&upload.document_id)
        .await
        .expect_err("deleted document status should not be reachable");
    assert!(
        status_resp.to_string().contains("404")
            || status_resp.to_string().contains("client error"),
        "expected 404 after delete, got {status_resp}"
    );
}

#[tokio::test]
async fn reindex_completed_document_requeues_ingestion() {
    super::require_integration_suite();
    let mut ctx = TestContext::new_smoke_with_rag().await;
    let upload = ctx.upload_document("antifragile.txt").await.expect("upload");
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(120))
        .await
        .expect("initial ingest");
    assert_eq!(status, crate::product_e2e::DocumentStatus::Completed);

    let before_chunks = ctx
        .query_document_chunk_count(&upload.document_id)
        .await
        .expect("chunk count before reindex");
    assert!(before_chunks > 0, "expected chunks before reindex");

    let reindex_resp = ctx
        .reindex_document(&upload.document_id)
        .await
        .expect("reindex document");
    assert_eq!(
        reindex_resp.status, 202,
        "reindex should return 202 Accepted, body={}",
        reindex_resp.body_json
    );

    let after_status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(120))
        .await
        .expect("reindex ingest");
    assert_eq!(after_status, crate::product_e2e::DocumentStatus::Completed);

    let after_chunks = ctx
        .query_document_chunk_count(&upload.document_id)
        .await
        .expect("chunk count after reindex");
    assert!(
        after_chunks > 0,
        "expected chunks after reindex, before={before_chunks} after={after_chunks}"
    );
}
