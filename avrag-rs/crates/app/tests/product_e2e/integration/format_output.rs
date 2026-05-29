//! P1-4 / P1-5: Format output — presentation-html and html-renderer.
//!
//! Product gaps blocking these tests:
//! 1. HTTP `ChatRequest` (contracts/src/chat.rs) has no `format_hint` field.
//! 2. `build_agent_request` (app/src/lib_impl/state_methods.rs:362) hard-codes
//!    `format_hint: None`, so format skills are never triggered via HTTP.
//! 3. `ChatResponse` has no `format_output` field; formatted content is only
//!    available in the raw `answer` string when triggered at the agent level.
//!
//! Once `format_hint` is added to the HTTP contract and wired through the
//! pipeline, these tests can be un-ignored and run against the same mock
//! infrastructure used by other integration tests.

use crate::product_e2e::{assertions::*, TestContext};

#[tokio::test]
#[ignore = "blocked: HTTP ChatRequest lacks format_hint field"]
async fn chat_presentation_html_returns_structured_slides() {
    let ctx = TestContext::new_smoke().await;

    // When format_hint is supported, this query should trigger the
    // presentation-html skill and return slide boundaries in the response.
    let _http_resp = ctx
        .chat(
            "Generate a presentation summarising Rust ownership",
            "test-notebook",
            &[],
        )
        .await
        .unwrap();

    // Expected assertions (to activate once product supports format_hint):
    // assert_http_ok(&http_resp);
    // let resp: ChatResponse = serde_json::from_value(http_resp.body_json).unwrap();
    // assert!(resp.answer.to_lowercase().contains("slide"));
    // assert!(resp.answer.to_lowercase().contains("<html") || resp.answer.to_lowercase().contains("<!doctype"));
}

#[tokio::test]
#[ignore = "blocked: HTTP ChatRequest lacks format_hint field"]
async fn chat_html_renderer_returns_valid_html() {
    let ctx = TestContext::new_smoke().await;

    // When format_hint is supported, this query should trigger the
    // html-renderer skill and return a well-formed HTML page.
    let _http_resp = ctx
        .chat(
            "Render an HTML page showing Rust error handling best practices",
            "test-notebook",
            &[],
        )
        .await
        .unwrap();

    // Expected assertions (to activate once product supports format_hint):
    // assert_http_ok(&http_resp);
    // let resp: ChatResponse = serde_json::from_value(http_resp.body_json).unwrap();
    // assert!(resp.answer.to_lowercase().contains("<html"));
    // assert!(resp.answer.to_lowercase().contains("<body"));
}
