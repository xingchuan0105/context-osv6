//! P0-3: Open query returns Search citation with source_type == "web".

use crate::product_e2e::{ChatResponse, HttpResponse, TestContext, assertions::*};

#[tokio::test]
async fn open_query_returns_web_citation() {
    let ctx = TestContext::new_smoke().await;

    let notebook = ctx.create_notebook("test-notebook").await.unwrap();
    let http_resp: HttpResponse = ctx
        .search("What is the weather in Tokyo today?", &notebook.id)
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
