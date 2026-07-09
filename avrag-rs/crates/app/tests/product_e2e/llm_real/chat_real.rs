//! Real-LLM general chat (no RAG / no search).

use crate::product_e2e::{
    TestContext,
    assertions::assert_answer_substantive,
    llm_real::{chat_general_with_retry, merge_llm_real_extra},
};

#[tokio::test]
#[ignore = "requires real LLM API key; run with --ignored --test-threads=1"]
async fn real_llm_general_chat_returns_substantive_answer() {
    super::require_nightly_suite();
    let ctx = TestContext::new_with_real_llm().await;
    let notebook = ctx.create_workspace("chat-real").await.expect("notebook");

    let result = chat_general_with_retry(
        &ctx,
        "In one sentence, what is the capital of France?",
        &notebook.id,
    )
    .await;
    let resp = &result.resp;

    assert_answer_substantive(resp, 10);
    let answer_lower = resp.answer.to_ascii_lowercase();
    assert!(
        answer_lower.contains("paris") || resp.answer.contains("巴黎"),
        "expected Paris/巴黎 in answer, got: {}",
        resp.answer
    );

    ctx.save_llm_artifact(
        "real_llm_general_chat_returns_substantive_answer",
        resp,
        merge_llm_real_extra(&result, None),
        Some(result.reasoning),
    );
}
