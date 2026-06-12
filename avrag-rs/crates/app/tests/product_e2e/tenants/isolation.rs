//! P2-14: Multi-tenant document isolation.
//!
//! Two tests verify that one org cannot read another org's documents,
//! enforced at the chat/RAG layer (not just at the API surface).
//!
//! - `cross_org_rag_does_not_leak_documents` — User B in org-B queries
//!   their own notebook. The notebook is empty, so RAG should either
//!   fail with `docscope_required` or return an answer that contains
//!   zero citations from org-A's documents.
//! - `cross_org_rag_cannot_query_org_a_doc_by_id` — User B explicitly
//!   passes org-A's `document_id` in `doc_scope`. RAG should refuse to
//!   use a foreign document (either 4xx error or the response must
//!   contain zero citations referencing the foreign doc).

use std::time::Duration;

use crate::product_e2e::assertions::assert_answer_excludes_keywords;
use crate::product_e2e::{ChatResponse, DocumentStatus, HttpResponse, TestContext};

const ORG_A: &str = "11111111-1111-1111-1111-111111111111";
const USER_A: &str = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
const ORG_B: &str = "22222222-2222-2222-2222-222222222222";
const USER_B: &str = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";

#[tokio::test]
async fn cross_org_rag_does_not_leak_documents() {
    super::require_integration_suite();

    // 1. User A (org-A) uploads and ingests a document
    let mut ctx_a = TestContext::new_smoke_with_rag_and_org(ORG_A, USER_A).await;
    let upload_a = ctx_a.upload_document("antifragile.txt").await.unwrap();
    let status_a = ctx_a
        .wait_for_ingestion(&upload_a.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(status_a, DocumentStatus::Completed);

    // 2. User B (org-B) creates their own notebook and queries the
    //    same topic. org-B has no documents, so RAG should NOT reach
    //    into org-A's data to find answers.
    let ctx_b = TestContext::new_smoke_with_rag_and_org(ORG_B, USER_B).await;
    let notebook_b = ctx_b.create_notebook("org-b-notebook").await.unwrap();
    let http_resp: HttpResponse = ctx_b
        .chat("What is antifragility?", &notebook_b.id, &[])
        .await
        .unwrap();

    // 3. Two acceptable outcomes:
    //    (a) Production rejects with a 4xx (e.g. docscope_required) — fine.
    //    (b) Production returns 200 but cites only org-B's docs (none here) — fine.
    //    What is NOT acceptable: 200 with citations pointing into org-A.
    if http_resp.status == 200 {
        let resp: ChatResponse = http_resp.into_business().unwrap();
        let leaked = resp
            .citations
            .iter()
            .any(|c| c.doc_id == upload_a.document_id);
        assert!(
            !leaked,
            "cross-org leak: org-B chat returned citation for org-A doc {}",
            upload_a.document_id
        );
        assert_answer_excludes_keywords(&resp, &["antifragile", "taleb"]);
    } else {
        // Acceptable: server rejected (e.g. docscope_required). Make sure
        // it is a client error, not a 5xx (which would mean RAG crashed).
        assert!(
            (400..500).contains(&http_resp.status),
            "expected 4xx when org-B has no docs, got HTTP {} body={}",
            http_resp.status,
            http_resp.body_json
        );
    }
}

#[tokio::test]
async fn cross_org_rag_cannot_query_org_a_doc_by_id() {
    super::require_integration_suite();

    // 1. User A (org-A) uploads and ingests a document
    let mut ctx_a = TestContext::new_smoke_with_rag_and_org(ORG_A, USER_A).await;
    let upload_a = ctx_a.upload_document("antifragile.txt").await.unwrap();
    let status_a = ctx_a
        .wait_for_ingestion(&upload_a.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(status_a, DocumentStatus::Completed);

    // 2. User B (org-B) tries to bypass auto-scope by passing org-A's
    //    document_id directly in doc_scope. A correct RAG layer must
    //    refuse to use a document owned by a different org.
    let ctx_b = TestContext::new_smoke_with_rag_and_org(ORG_B, USER_B).await;
    let notebook_b = ctx_b.create_notebook("org-b-notebook").await.unwrap();
    let http_resp: HttpResponse = ctx_b
        .chat(
            "What is antifragility?",
            &notebook_b.id,
            &[upload_a.document_id.clone()],
        )
        .await
        .unwrap();

    // 3. Passing a foreign doc_id in doc_scope MUST be rejected with 4xx.
    //    HTTP 200 is no longer acceptable — the server should refuse outright.
    assert!(
        (400..500).contains(&http_resp.status),
        "expected 4xx when org-B passes foreign doc_scope, got HTTP {} body={}",
        http_resp.status,
        http_resp.body_json
    );
}
