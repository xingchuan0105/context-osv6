//! P0: General/chat agent smoke — mock LLM routes to ChatAnswer.
//! G-17: pure-chat utility tools (calculator) under Option D AnswerOnly.

use crate::product_e2e::{ChatResponse, TestContext, assertions::*};

#[tokio::test]
async fn general_agent_returns_non_empty_answer() {
    super::require_smoke_suite();
    let ctx = TestContext::new_smoke().await;
    let notebook = ctx.create_workspace("chat-smoke").await.unwrap();

    let http_resp = ctx
        .chat_general("Hello, who are you?", &notebook.id)
        .await
        .unwrap();

    assert_http_ok(&http_resp);
    assert!(http_resp.status < 500, "general chat must not 5xx");

    let resp: ChatResponse = http_resp.into_business().unwrap();
    assert_observability_contract(&resp);
    assert_eq!(
        resp.agent_type, "chat",
        "chat agent_type expected, got {}",
        resp.agent_type
    );
    assert_answer_substantive(&resp, 10);
}

/// G-17: pure chat (no capabilities) must expose calculator in the utility pool
/// and, under mock LLM, invoke it for arithmetic then answer with the result.
#[tokio::test]
async fn pure_chat_calculator_utility_tool_is_invoked() {
    super::require_smoke_suite();
    let ctx = TestContext::new_smoke().await;
    let notebook = ctx.create_workspace("chat-calc-smoke").await.unwrap();

    // 1+2*3 = 7 — classic G-17 probe.
    let http_resp = ctx
        .chat_general("请计算：1+2*3 等于多少？", &notebook.id)
        .await
        .unwrap();

    assert_http_ok(&http_resp);
    assert!(http_resp.status < 500, "calculator chat must not 5xx: {http_resp:?}");

    let resp: ChatResponse = http_resp.into_business().unwrap();
    assert_eq!(resp.agent_type, "chat");
    let tools: Vec<&str> = resp.tool_results.iter().map(|t| t.tool.as_str()).collect();
    assert!(
        tools.iter().any(|t| *t == "calculator"),
        "pure chat utility path must call calculator; tool_results={tools:?} answer={}",
        resp.answer
    );
    assert!(
        resp.answer.contains('7'),
        "answer should include calculator result 7, got: {}",
        resp.answer
    );
}

/// G-17 companion: golden-style 128×46+357 → 6245 under mock.
#[tokio::test]
async fn pure_chat_calculator_matches_golden_builtin_expression() {
    super::require_smoke_suite();
    let ctx = TestContext::new_smoke().await;
    let notebook = ctx.create_workspace("chat-calc-6245").await.unwrap();

    let http_resp = ctx
        .chat_general("请计算：128×46+357 等于多少？", &notebook.id)
        .await
        .unwrap();

    assert_http_ok(&http_resp);
    let resp: ChatResponse = http_resp.into_business().unwrap();
    let tools: Vec<&str> = resp.tool_results.iter().map(|t| t.tool.as_str()).collect();
    assert!(
        tools.iter().any(|t| *t == "calculator"),
        "expected calculator tool, got {tools:?}"
    );
    assert!(
        resp.answer.contains("6245"),
        "expected 6245 in answer, got: {}",
        resp.answer
    );
}
