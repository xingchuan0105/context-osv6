//! P0-3: Open query returns Search citation with source_type == "web".

use crate::product_e2e::{assertions::*, ChatResponse, HttpResponse, TestContext};

#[tokio::test]
#[ignore = "Phase 1 — requires Mock Search provider injection + working Search strategy"]
async fn open_query_returns_web_citation() {
    let ctx = TestContext::new_smoke().await;

    // Query with empty doc_scope forces Search path (no documents to RAG over)
    let http_resp: HttpResponse = ctx
        .chat("What is the weather in Tokyo today?", &[])
        .await
        .unwrap();

    // Protocol assertions
    assert_http_ok(&http_resp);

    // Deserialize
    let resp: ChatResponse = http_resp.into_business().unwrap();

    // Product assertions
    assert_has_citations(&resp);
    assert_answer_has_web_citation(&resp);
}
