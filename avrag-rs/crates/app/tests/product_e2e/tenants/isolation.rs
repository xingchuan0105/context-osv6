//! P2-14: Multi-tenant document isolation.
//!
//! User A (org-1) uploads a document. User B (org-2) queries the same topic.
//! User B must NOT receive citations pointing to User A's document.

use std::time::Duration;

use crate::product_e2e::{assertions::*, ChatResponse, DocumentStatus, HttpResponse, TestContext};

#[tokio::test]
#[ignore = "requires per-org Milvus isolation + dynamic auth switching in TestContext"]
async fn cross_org_rag_does_not_leak_documents() {
    let ctx = TestContext::new_smoke_with_rag().await;

    // 1. User A uploads a document
    let upload_a = ctx.upload_document("antifragile.txt").await.unwrap();
    let status_a = ctx
        .wait_for_ingestion(&upload_a.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(status_a, DocumentStatus::Completed);

    // 2. User B queries their own notebook (empty doc_scope forces RAG clarify,
    //    but with a doc from org B the isolation would be tested).
    //    For now we simulate User B with the same client — the real test needs
    //    a second TestContext with different org_id or auth header override.
    let http_resp: HttpResponse = ctx
        .chat("What is antifragility?", &upload_a.notebook_id, &[upload_a.document_id.clone()])
        .await
        .unwrap();

    assert_http_ok(&http_resp);
    let resp: ChatResponse = http_resp.into_business().unwrap();

    // If org isolation is working, User B should only see their own documents.
    // With the current single-org test setup this assertion is a placeholder.
    let has_foreign_doc = resp
        .citations
        .iter()
        .any(|c| c.doc_id != upload_a.document_id);
    assert!(
        !has_foreign_doc,
        "cross-org citation leak detected: got citation from doc_id not in user's scope"
    );
}
