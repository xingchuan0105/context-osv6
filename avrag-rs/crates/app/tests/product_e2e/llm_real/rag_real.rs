//! Real-LLM RAG E2E regression tests.
//!
//! These tests validate that the V5 skill-based prompt assembly still
//! produces coherent RAG behavior against production LLM providers.
//!
//! Run:
//!   cargo test -p app --test product_e2e llm_real::rag_real -- --ignored --test-threads=1 --nocapture

use std::time::Duration;

use crate::product_e2e::{
    ChatResponse, DocumentStatus, HttpResponse, TestContext,
    assertions::{assert_answer_has_doc_citation, assert_http_ok},
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

    // 3. Ask a question that requires reading the document.
    let http_resp: HttpResponse = ctx
        .chat(
            "What is antifragility?",
            &upload.notebook_id,
            &[upload.document_id.clone()],
        )
        .await
        .expect("chat request");

    // 4. Protocol + product assertions (loose, because LLM output is non-deterministic).
    assert_http_ok(&http_resp);

    let resp: ChatResponse = http_resp
        .into_business()
        .expect("valid ChatResponse schema");

    assert!(
        !resp.answer.is_empty(),
        "real LLM should produce a non-empty answer"
    );
    assert!(
        resp.answer.to_lowercase().contains("antifragil")
            || resp.answer.to_lowercase().contains("taleb"),
        "answer should mention the topic or author; got: {}",
        resp.answer
    );
    assert!(
        resp.degrade_trace.is_empty(),
        "expected no degradation trace on the happy path, got: {:?}",
        resp.degrade_trace
    );

    // 5. Hard assertion: RAG must return citations on the happy path.
    assert!(
        !resp.citations.is_empty(),
        "real-LLM RAG returned zero citations on the happy path; answer={}",
        resp.answer
    );
    assert_answer_has_doc_citation(&resp);
    let cites_uploaded = resp
        .citations
        .iter()
        .any(|c| c.doc_id == upload.document_id);
    assert!(
        cites_uploaded,
        "expected at least one citation from uploaded doc {}, got: {:?}",
        upload.document_id, resp.citations
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
