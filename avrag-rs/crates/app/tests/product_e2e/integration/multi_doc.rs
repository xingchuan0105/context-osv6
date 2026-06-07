//! P1-6: Multi-document RAG — citations span ≥2 documents.

use std::collections::HashSet;
use std::time::Duration;

use crate::product_e2e::{ChatResponse, DocumentStatus, HttpResponse, TestContext, assertions::*};

#[tokio::test]
async fn multi_doc_rag_returns_citations_from_both_docs() {
    let mut ctx = TestContext::new_smoke_with_rag().await;
    let notebook = ctx.create_notebook("multi-doc-test").await.unwrap();

    // 1. Upload two documents to the same notebook
    let upload1 = ctx
        .upload_document_to_notebook("antifragile.txt", &notebook.id)
        .await
        .unwrap();
    let upload2 = ctx
        .upload_document_to_notebook("lindy.txt", &notebook.id)
        .await
        .unwrap();

    // 2. Wait for both ingestions
    let status1 = ctx
        .wait_for_ingestion(&upload1.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(status1, DocumentStatus::Completed);
    let status2 = ctx
        .wait_for_ingestion(&upload2.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(status2, DocumentStatus::Completed);

    // 3. Query across both documents
    let doc_scope = vec![upload1.document_id.clone(), upload2.document_id.clone()];
    let http_resp: HttpResponse = ctx
        .chat("What are the key concepts?", &notebook.id, &doc_scope)
        .await
        .unwrap();

    // 4. Protocol assertions
    assert_http_ok(&http_resp);

    // 5. Business assertions
    let resp: ChatResponse = http_resp.into_business().unwrap();
    assert_has_citations(&resp);
    assert_answer_substantive(&resp, 50);

    // 6. Multi-doc assertion: citations must come from ≥2 distinct doc_ids
    let unique_doc_ids: HashSet<&str> = resp.citations.iter().map(|c| c.doc_id.as_str()).collect();
    assert!(
        unique_doc_ids.len() >= 2,
        "expected citations from >=2 distinct documents, got doc_ids: {:?}",
        unique_doc_ids
    );
}
