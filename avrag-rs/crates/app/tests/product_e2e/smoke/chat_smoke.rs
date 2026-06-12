//! P0: General/chat agent smoke — mock LLM routes to ChatAnswer.

use crate::product_e2e::{ChatResponse, TestContext, assertions::*};

#[tokio::test]
async fn general_agent_returns_non_empty_answer() {
    let ctx = TestContext::new_smoke().await;
    let notebook = ctx.create_notebook("chat-smoke").await.unwrap();

    let http_resp = ctx
        .chat_general("Hello, who are you?", &notebook.id)
        .await
        .unwrap();

    assert_http_ok(&http_resp);
    assert!(http_resp.status < 500, "general chat must not 5xx");

    let resp: ChatResponse = http_resp.into_business().unwrap();
    assert_observability_contract(&resp);
    assert_eq!(resp.agent_type, "chat", "chat agent_type expected, got {}", resp.agent_type);
    assert_answer_substantive(&resp, 10);
}
