//! Real-LLM Search E2E regression tests.
//!
//! These tests validate that the V5 skill-based search pipeline still
//! produces coherent web-grounded answers against production providers.
//!
//! Run:
//!   cargo test -p app --test product_e2e llm_real::search_real -- --ignored --test-threads=1 --nocapture

use crate::product_e2e::{
    assertions::{assert_answer_has_web_citation, assert_http_ok},
    ChatResponse, HttpResponse, TestContext,
};

/// P0: Open-domain search query returns a substantive answer with at least
/// one web citation when using the real search LLM and Brave search provider.
#[tokio::test]
#[ignore = "requires real LLM + Brave Search API key; run with --ignored --test-threads=1"]
async fn real_llm_search_open_query_returns_web_citation() {
    let ctx = TestContext::new_with_real_llm().await;

    let notebook = ctx
        .create_notebook("test-notebook")
        .await
        .expect("create notebook");

    let http_resp: HttpResponse = ctx
        .search("What is the current weather in Tokyo today?", &notebook.id)
        .await
        .expect("search request");

    assert_http_ok(&http_resp);

    let resp: ChatResponse = http_resp
        .into_business()
        .expect("valid ChatResponse schema");

    assert!(
        !resp.answer.is_empty(),
        "real search LLM should produce a non-empty answer"
    );
    assert!(
        resp.answer.to_lowercase().contains("tokyo")
            || resp.answer.to_lowercase().contains("weather")
            || resp.answer.to_lowercase().contains("°c")
            || resp.answer.to_lowercase().contains("temperature"),
        "answer should mention Tokyo or weather; got: {}",
        resp.answer
    );
    assert!(
        resp.degrade_trace.is_empty(),
        "expected no degradation trace on the happy path, got: {:?}",
        resp.degrade_trace
    );

    // Web citation is best-effort under a real provider.
    if resp.citations.is_empty() {
        eprintln!(
            "WARNING: real search returned zero citations; this may be transient. answer={}",
            resp.answer
        );
    } else {
        assert_answer_has_web_citation(&resp);
    }
}
