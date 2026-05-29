//! P0-2: Document Q&A returns structured citation.

use std::time::Duration;

use crate::product_e2e::{assertions::*, ChatResponse, DocumentStatus, HttpResponse, TestContext};

#[tokio::test]
#[ignore = "Phase 1 — requires TestContext::chat + working RAG pipeline"]
async fn rag_document_qa_returns_citation() {
    let ctx = TestContext::new_smoke().await;

    // 1. Upload document
    let upload = ctx.upload_document("antifragile.txt").await.unwrap();
    assert_eq!(upload.status, 202);

    // 2. Wait for ingestion
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(30))
        .await
        .unwrap();
    assert_eq!(status, DocumentStatus::Completed);

    // 3. Chat — returns HttpResponse (protocol layer)
    let http_resp: HttpResponse = ctx
        .chat("What is antifragility?", &[upload.document_id.clone()])
        .await
        .unwrap();

    // 4. Protocol assertions
    assert_http_ok(&http_resp);

    // 5. Deserialize to business object
    let resp: ChatResponse = http_resp.into_business().unwrap();

    // 6. Product assertions
    assert_has_citations(&resp);
    assert_citation_doc_id(&resp, &upload.document_id);
    assert_answer_has_doc_citation(&resp);
    assert_answer_substantive(&resp, 50);
}
