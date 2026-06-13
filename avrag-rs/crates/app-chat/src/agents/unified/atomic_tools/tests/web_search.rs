use super::*;
use crate::agents::unified::atomic_tools::dispatch_atomic_tool;
use contracts::ToolStatus;

#[tokio::test]
async fn test_web_search_basic() {
    let call = tool_call("web_search", serde_json::json!({"query": "rust lang"}));
    let provider = FakeSearchProvider;
    let result = dispatch_atomic_tool(&call, Some(&provider)).await;
    assert_eq!(result.status, ToolStatus::Ok);
    let data = result.data.unwrap();
    assert_eq!(data["query_type"], "test");
    assert_eq!(data["sub_queries"].as_array().unwrap().len(), 1);
    let results = data["results"].as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["title"], "Result for rust lang");
}

#[tokio::test]
async fn test_web_search_no_provider() {
    let call = tool_call("web_search", serde_json::json!({"query": "rust"}));
    let result = dispatch_atomic_tool(&call, None).await;
    assert_eq!(result.status, ToolStatus::Error);
    let data = result.data.unwrap();
    assert!(data["error"].as_str().unwrap().contains("not available"));
}

#[tokio::test]
async fn test_web_search_missing_query() {
    let call = tool_call("web_search", serde_json::json!({}));
    let provider = FakeSearchProvider;
    let result = dispatch_atomic_tool(&call, Some(&provider)).await;
    assert_eq!(result.status, ToolStatus::Error);
    let data = result.data.unwrap();
    assert!(data["error"].as_str().unwrap().contains("missing query"));
}

#[tokio::test]
async fn test_web_search_with_vertical() {
    let call = tool_call(
        "web_search",
        serde_json::json!({"query": "news", "vertical": "news"}),
    );
    let provider = FakeSearchProvider;
    let result = dispatch_atomic_tool(&call, Some(&provider)).await;
    assert_eq!(result.status, ToolStatus::Ok);
}
