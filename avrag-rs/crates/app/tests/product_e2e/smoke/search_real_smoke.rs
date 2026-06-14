//! Staging smoke: real Brave Search via `SEARCH_USE_REAL=1` (ignored in PR CI).

use crate::product_e2e::{
    ChatResponse, HttpResponse, TestContext,
    assertions::{assert_answer_has_web_citation, assert_has_citations},
    llm_real::{ensure_search_defaults, has_real_search_credentials, load_env_from_repo_dotenv},
};

#[tokio::test]
#[ignore = "requires SEARCH_API_KEY + Brave reachable; run with --ignored for staging preflight"]
async fn open_query_with_real_brave_returns_web_citation() {
    super::require_smoke_suite();
    load_env_from_repo_dotenv();
    assert!(
        has_real_search_credentials(),
        "SEARCH_API_KEY (or E2E_BRAVE_API_KEY) required for real search smoke"
    );
    ensure_search_defaults();
    unsafe { std::env::set_var("SEARCH_USE_REAL", "1") };

    let ctx = TestContext::new_smoke().await;
    let notebook = ctx.create_notebook("real-search-smoke").await.unwrap();
    let http_resp: HttpResponse = ctx
        .search("What is the weather in Tokyo today?", &notebook.id)
        .await
        .expect("real search chat");

    assert_eq!(
        http_resp.status, 200,
        "real search smoke must return HTTP 200, body={}",
        http_resp.body_json
    );
    let resp: ChatResponse = http_resp.into_business().unwrap();
    assert_has_citations(&resp);
    assert_answer_has_web_citation(&resp);
    assert!(
        resp.degrade_trace.is_empty(),
        "real Brave search happy path should not degrade, got {:?}",
        resp.degrade_trace
    );
}
