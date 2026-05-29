//! P2-10: Search provider 429 → degrade gracefully.

use crate::product_e2e::{assertions::*, ChatResponse, HttpResponse, TestContext};

#[tokio::test]
async fn search_429_returns_degraded_answer() {
    let ctx = TestContext::new_smoke().await;

    // 1. Force mock search to return 429
    ctx.set_search_429(true);

    let notebook = ctx.create_notebook("test-notebook").await.unwrap();
    let http_resp: HttpResponse = ctx
        .search("What is the weather in Tokyo today?", &notebook.id)
        .await
        .unwrap();

    // 2. Protocol: HTTP 200 (internal errors must not leak)
    assert_http_ok(&http_resp);

    // 3. Business: no web citations, degrade trace present
    let resp: ChatResponse = http_resp.into_business().unwrap();
    let has_web = resp.citations.iter().any(|c| c.layer.as_deref() == Some("search"));
    assert!(!has_web, "should not have web citation when search is 429");
    assert!(
        !resp.degrade_trace.is_empty(),
        "expected degrade_trace when search fails, got none"
    );
}
