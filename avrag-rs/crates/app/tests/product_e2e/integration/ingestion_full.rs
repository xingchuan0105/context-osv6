//! P1: Full ingestion pipeline edge cases.

use std::time::Duration;

use crate::product_e2e::{DocumentStatus, HttpResponse, TestContext};

#[tokio::test]
async fn empty_document_ingests_with_zero_chunks_and_degrades() {
    super::require_integration_suite();

    let mut ctx = TestContext::new_smoke_with_rag().await;

    // 1. Upload empty document
    let upload = ctx.upload_document("empty.txt").await.unwrap();
    assert_eq!(
        upload.status, 201,
        "expected HTTP 201 from POST .../documents"
    );

    // 2. Empty index is terminal integrity (S4): must not hang in queued forever.
    // Worker dead-letters EmptyIndex immediately → document Failed (not requeue/backoff).
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(60))
        .await
        .unwrap();
    assert_eq!(
        status,
        DocumentStatus::Failed,
        "empty index must mark document Failed (not completed with empty body / infinite requeue)"
    );

    // 3. Failed docs are rejected from RAG doc_scope (invalid_doc_scope) — not 5xx.
    let http_resp: HttpResponse = ctx
        .chat(
            "What is in this document?",
            &upload.workspace_id,
            &[upload.document_id.clone()],
        )
        .await
        .unwrap();
    assert_eq!(
        http_resp.status, 400,
        "failed empty doc in doc_scope must be 400, body: {}",
        http_resp.body_json
    );
    let err = http_resp
        .body_json
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert_eq!(
        err, "invalid_doc_scope",
        "expected invalid_doc_scope for failed document, body: {}",
        http_resp.body_json
    );
}
