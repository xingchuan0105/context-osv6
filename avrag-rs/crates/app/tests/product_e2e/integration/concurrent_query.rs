//! P2-13: Concurrent queries against the same document produce independent results.

use std::time::Duration;

use crate::product_e2e::{ChatResponse, DocumentStatus, TestContext, assertions::*};

use super::require_integration_suite;

#[tokio::test]
async fn concurrent_rag_queries_return_independent_citations() {
    require_integration_suite();

    let mut ctx = TestContext::new_smoke_with_rag().await;

    let upload = ctx.upload_document("antifragile.txt").await.unwrap();
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(status, DocumentStatus::Completed);

    let chunk_count = ctx
        .query_document_chunk_count(&upload.document_id)
        .await
        .unwrap();
    assert!(
        chunk_count > 0,
        "expected chunk_count > 0 after successful ingestion, got {chunk_count}"
    );

    let doc_scope = vec![upload.document_id.clone()];
    let (http1, http2) = tokio::join!(
        ctx.chat_without_mock_chunk_pin(
            "What is antifragility?",
            &upload.notebook_id,
            &doc_scope,
        ),
        ctx.chat_without_mock_chunk_pin(
            "What is the Lindy Effect described in this document?",
            &upload.notebook_id,
            &doc_scope,
        ),
    );
    let http1 = http1.unwrap();
    let http2 = http2.unwrap();

    assert_http_ok(&http1);
    assert_http_ok(&http2);

    let chat1: ChatResponse = http1.into_business().unwrap();
    let chat2: ChatResponse = http2.into_business().unwrap();

    assert_codegen_bridge_dense_retrieval(&chat1);
    assert_codegen_bridge_dense_retrieval(&chat2);
    assert_has_citations(&chat1);
    assert_has_citations(&chat2);
    assert_answer_substantive(&chat1, 30);
    assert_answer_substantive(&chat2, 30);

    assert_citation_doc_id(&chat1, &upload.document_id);
    assert_citation_doc_id(&chat2, &upload.document_id);
    assert_independent_citation_chunks(&chat1, &chat2);

    assert_ne!(
        chat1.answer, chat2.answer,
        "concurrent queries on different topics should produce distinct answers"
    );
    let answer1_lower = chat1.answer.to_lowercase();
    let answer2_lower = chat2.answer.to_lowercase();
    assert!(
        answer1_lower.contains("antifrag") || answer1_lower.contains("fragile"),
        "first answer should address antifragility, got: {}",
        chat1.answer
    );
    assert!(
        answer2_lower.contains("lindy"),
        "second answer should address Lindy Effect, got: {}",
        chat2.answer
    );
}
