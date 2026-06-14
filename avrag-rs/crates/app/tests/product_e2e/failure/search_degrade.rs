//! Search timeout and empty-result degradation (mock Brave).

use crate::product_e2e::{ChatResponse, HttpResponse, TestContext, assertions::*};

#[tokio::test]
async fn search_timeout_returns_degraded_answer() {
    super::require_integration_suite();

    unsafe { std::env::set_var("SEARCH_TIMEOUT_MS", "500") };
    let ctx = TestContext::new_smoke().await;
    ctx.set_search_delay_ms(2_000);

    let notebook = ctx.create_notebook("search-timeout").await.unwrap();
    let http_resp: HttpResponse = ctx
        .search("What is the weather in Tokyo today?", &notebook.id)
        .await
        .unwrap();

    assert_http_ok(&http_resp);
    let resp: ChatResponse = http_resp.into_business().unwrap();
    let has_web = resp
        .citations
        .iter()
        .any(|c| c.layer.as_deref() == Some("search"));
    assert!(!has_web, "timed-out search must not produce web citations");
    assert!(
        !resp.degrade_trace.is_empty(),
        "expected degrade_trace when search times out, got none"
    );
}

#[tokio::test]
async fn search_empty_results_returns_degraded_answer() {
    super::require_integration_suite();

    let ctx = TestContext::new_smoke().await;
    ctx.set_search_empty(true);

    let notebook = ctx.create_notebook("search-empty").await.unwrap();
    let http_resp: HttpResponse = ctx
        .search("obscure query with no brave hits xyz123", &notebook.id)
        .await
        .unwrap();

    assert_http_ok(&http_resp);
    let resp: ChatResponse = http_resp.into_business().unwrap();
    let has_web = resp
        .citations
        .iter()
        .any(|c| c.layer.as_deref() == Some("search"));
    assert!(!has_web, "empty search results must not produce web citations");
    assert!(
        resp.answer.contains("could not retrieve web evidence")
            || !resp.degrade_trace.is_empty()
            || resp.citations.is_empty(),
        "expected degraded search answer, got answer={} degrade={:?}",
        resp.answer,
        resp.degrade_trace
    );
}
