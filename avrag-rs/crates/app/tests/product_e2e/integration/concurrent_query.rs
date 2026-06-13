//! P2-13: Concurrent queries against the same document produce independent results.

use std::time::Duration;

use crate::product_e2e::{ChatResponse, DocumentStatus, TestContext, assertions::*};

use super::require_integration_suite;

#[tokio::test]
async fn concurrent_rag_queries_are_safe_on_codegen_bridge() {
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

    // Mock LLM synthesis is query-agnostic; this case validates concurrent safety of the
    // codegen → dense_search bridge rather than answer/chunk differentiation.
}

/// Real-LLM gate for citation-chunk independence under concurrent RAG queries.
///
/// Run: `E2E_MODE=nightly cargo test -p app --test product_e2e \
///   integration::concurrent_query::real_llm_concurrent_rag_queries_have_independent_citation_chunks \
///   -- --ignored --test-threads=1 --nocapture`
#[tokio::test]
#[ignore = "requires real LLM API key; run with --ignored --test-threads=1"]
async fn real_llm_concurrent_rag_queries_have_independent_citation_chunks() {
    crate::product_e2e::llm_real::require_nightly_suite();

    let mut ctx = TestContext::new_with_real_llm().await;

    let upload = ctx.upload_document("antifragile.txt").await.unwrap();
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(180))
        .await
        .unwrap();
    assert_eq!(status, DocumentStatus::Completed);

    let doc_scope = vec![upload.document_id.clone()];
    let (result1, result2) = tokio::join!(
        crate::product_e2e::llm_real::chat_with_citations_retry(
            &ctx,
            "What is antifragility?",
            &upload.notebook_id,
            &doc_scope,
        ),
        crate::product_e2e::llm_real::chat_with_citations_retry(
            &ctx,
            "What is the Lindy Effect described in this document?",
            &upload.notebook_id,
            &doc_scope,
        ),
    );

    assert_independent_citation_chunks(&result1.resp, &result2.resp);
    assert_citation_doc_id(&result1.resp, &upload.document_id);
    assert_citation_doc_id(&result2.resp, &upload.document_id);

    ctx.save_llm_artifact(
        "real_llm_concurrent_rag_queries_have_independent_citation_chunks",
        &result1.resp,
        Some(serde_json::json!({
            "concurrent_query_a": "What is antifragility?",
            "concurrent_query_b": "What is the Lindy Effect described in this document?",
            "peer_answer_len": result2.resp.answer.len(),
            "peer_citation_count": result2.resp.citations.len(),
        })),
        None,
    );
}
