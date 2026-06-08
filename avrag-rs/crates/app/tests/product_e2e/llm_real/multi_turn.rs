//! Real-LLM multi-turn RAG E2E regression tests.
//!
//! Run:
//!   cargo test -p app --test product_e2e llm_real::multi_turn -- --ignored --test-threads=1 --nocapture

use std::time::Duration;

use crate::product_e2e::{
    DocumentStatus, TestContext,
    assertions::{assert_answer_substantive, assert_has_citations},
    llm_real::{chat_with_retry, chat_with_session_retry},
};

/// Turn 1: document-grounded RAG. Turn 2: follow-up in same session references Taleb.
#[tokio::test]
#[ignore = "requires real LLM API key; run with --ignored --test-threads=1"]
async fn real_llm_multi_turn_rag_follow_up_remembers_context() {
    let mut ctx = TestContext::new_with_real_llm().await;

    let upload = ctx
        .upload_document("antifragile.txt")
        .await
        .expect("upload document");
    assert_eq!(upload.status, 201);

    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(180))
        .await
        .expect("ingest document");
    assert_eq!(status, DocumentStatus::Completed);

    let doc_scope = vec![upload.document_id.clone()];

    let (_http1, resp1) = chat_with_retry(
        &ctx,
        "What is antifragility?",
        &upload.notebook_id,
        &doc_scope,
    )
    .await;

    assert_has_citations(&resp1);
    assert_answer_substantive(&resp1, 50);
    assert!(
        resp1.degrade_trace.is_empty(),
        "turn 1 degrade_trace: {:?}",
        resp1.degrade_trace
    );

    let session_id = resp1.session_id.clone();
    let (_http2, resp2) = chat_with_session_retry(
        &ctx,
        "Who wrote the book about it?",
        &upload.notebook_id,
        &doc_scope,
        &session_id,
    )
    .await;

    assert_answer_substantive(&resp2, 20);
    let answer_lower = resp2.answer.to_lowercase();
    assert!(
        answer_lower.contains("taleb"),
        "expected turn-2 answer to mention Taleb, got: {}",
        resp2.answer.chars().take(200).collect::<String>()
    );
    assert!(
        resp2.degrade_trace.is_empty(),
        "turn 2 degrade_trace: {:?}",
        resp2.degrade_trace
    );

    ctx.save_llm_artifact(
        "real_llm_multi_turn_rag_follow_up_remembers_context",
        &resp2,
        Some(serde_json::json!({
            "document_id": upload.document_id,
            "session_id": session_id,
            "turn1_answer_len": resp1.answer.len(),
        })),
    );
}
