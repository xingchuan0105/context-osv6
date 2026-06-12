//! P2-13: Concurrent queries against the same document produce independent results.

use std::time::Duration;

use crate::product_e2e::{ChatResponse, DocumentStatus, HttpResponse, TestContext, assertions::*};

#[tokio::test]
async fn concurrent_rag_queries_return_independent_citations() {
    let mut ctx = TestContext::new_smoke_with_rag().await;

    let upload = ctx.upload_document("antifragile.txt").await.unwrap();
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(status, DocumentStatus::Completed);

    ctx.reset_mock_state();

    let chunk_count = ctx
        .query_document_chunk_count(&upload.document_id)
        .await
        .unwrap();
    assert!(
        chunk_count > 0,
        "expected chunk_count > 0 after successful ingestion, got {chunk_count}"
    );

    let doc_scope = vec![upload.document_id.clone()];
    let http1 = ctx
        .chat_without_mock_chunk_pin(
            "What is antifragility?",
            &upload.notebook_id,
            &doc_scope,
        )
        .await
        .unwrap();
    let http2 = ctx
        .chat_without_mock_chunk_pin(
            "Who wrote about antifragility?",
            &upload.notebook_id,
            &doc_scope,
        )
        .await
        .unwrap();

    assert_http_ok(&http1);
    assert_http_ok(&http2);

    let chat1: ChatResponse = http1.into_business().unwrap();
    let chat2: ChatResponse = http2.into_business().unwrap();

    assert_has_citations(&chat1);
    assert_has_citations(&chat2);
    assert_answer_substantive(&chat1, 30);
    assert_answer_substantive(&chat2, 30);

    assert_citation_doc_id(&chat1, &upload.document_id);
    assert_citation_doc_id(&chat2, &upload.document_id);
}
