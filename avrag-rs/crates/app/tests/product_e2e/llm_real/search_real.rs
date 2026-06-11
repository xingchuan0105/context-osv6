//! Real-LLM Search E2E regression tests.
//!
//! These tests validate that the V5 skill-based search pipeline still
//! produces coherent web-grounded answers against production providers.
//!
//! Run:
//!   cargo test -p app --test product_e2e llm_real::search_real -- --ignored --test-threads=1 --nocapture

use crate::product_e2e::{
    TestContext,
    assertions::{assert_answer_has_web_citation, assert_answer_substantive, assert_has_citations},
    llm_real::{merge_llm_real_extra, search_with_retry},
};

/// P0: Open-domain search query returns a substantive answer with at least
/// one web citation when using the real search LLM and Brave search provider.
#[tokio::test]
#[ignore = "requires real LLM + Brave Search API key; run with --ignored --test-threads=1"]
async fn real_llm_search_open_query_returns_web_citation() {
    if std::env::var("SEARCH_REQUIRE_REAL").is_err() {
        unsafe { std::env::set_var("SEARCH_REQUIRE_REAL", "1") };
    }
    let ctx = TestContext::new_with_real_llm().await;

    let notebook = ctx
        .create_notebook("test-notebook")
        .await
        .expect("create notebook");

    let result = search_with_retry(
        &ctx,
        "What is the current weather in Tokyo today?",
        &notebook.id,
    )
    .await;
    let resp = &result.resp;

    // Product assertions — align with smoke/search_smoke: web citations + substance.
    assert_has_citations(resp);
    assert_answer_has_web_citation(resp);
    assert_answer_substantive(resp, 30);
    assert!(
        resp.degrade_trace.is_empty(),
        "expected no degradation trace on the happy path, got: {:?}",
        resp.degrade_trace
    );

    // Persist artifact for audit even on pass.
    ctx.save_llm_artifact(
        "real_llm_search_open_query_returns_web_citation",
        resp,
        merge_llm_real_extra(&result, None),
        Some(result.reasoning),
    );
}
