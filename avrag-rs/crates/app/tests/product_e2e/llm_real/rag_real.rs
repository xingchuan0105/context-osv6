//! Real-LLM RAG E2E regression tests.
//!
//! These tests validate that the V5 skill-based prompt assembly still
//! produces coherent RAG behavior against production LLM providers.
//!
//! Run:
//!   cargo test -p app --test product_e2e llm_real::rag_real -- --ignored --test-threads=1 --nocapture

use std::time::Duration;

use crate::product_e2e::{
    DocumentStatus, TestContext,
    assertions::{
        assert_answer_has_doc_citation, assert_answer_substantive, assert_citation_doc_id,
        assert_has_citations,
    },
    llm_real::chat_with_retry,
};

/// P0: Basic RAG document Q&A returns a substantive answer with at least
/// one document citation when using a real LLM and real embedding provider.
#[tokio::test]
#[ignore = "requires real LLM API key; run with --ignored --test-threads=1"]
async fn real_llm_rag_document_qa_returns_citation() {
    let mut ctx = TestContext::new_with_real_llm().await;

    // 1. Upload a fixture document.
    let upload = ctx
        .upload_document("antifragile.txt")
        .await
        .expect("upload document");
    assert_eq!(
        upload.status, 201,
        "expected HTTP 201 from POST .../documents"
    );

    // 2. Wait for real ingestion + embedding pipeline.
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(180))
        .await
        .expect("ingest document");
    assert_eq!(status, DocumentStatus::Completed);

    // 3. Ask a question that requires reading the document (retry for transient LLM errors).
    let (_http_resp, resp) = chat_with_retry(
        &ctx,
        "What is antifragility?",
        &upload.notebook_id,
        &[upload.document_id.clone()],
    )
    .await;

    // 4. Product assertions — align with smoke/rag_smoke: citations + substance, not keywords.
    assert_has_citations(&resp);
    assert_citation_doc_id(&resp, &upload.document_id);
    assert_answer_has_doc_citation(&resp);
    assert_answer_substantive(&resp, 50);
    assert!(
        resp.degrade_trace.is_empty(),
        "expected no degradation trace on the happy path, got: {:?}",
        resp.degrade_trace
    );

    // 6. White-box: verify the planner actually selected dense_retrieval.
    ctx.assert_tool_called("dense_retrieval");

    // 7. Persist artifact for audit even on pass.
    ctx.save_llm_artifact(
        "real_llm_rag_document_qa_returns_citation",
        &resp,
        Some(serde_json::json!({"document_id": upload.document_id})),
    );
}
