use super::*;
use crate::agents::unified::atomic_tools::{
    dispatch_atomic_tool, dispatch_atomic_tools, dispatch_atomic_tools_with_provider,
};
use contracts::ToolStatus;

#[tokio::test]
async fn test_dispatch_multiple_tools_parallel() {
    let calls = vec![
        tool_call("calculator", serde_json::json!({"expression": "1+1"})),
        tool_call("calculator", serde_json::json!({"expression": "2*3"})),
    ];
    let results = dispatch_atomic_tools(calls).await;
    assert_eq!(results.len(), 2);
    assert_eq!(
        results[0].data.as_ref().unwrap()["result"]
            .as_f64()
            .unwrap(),
        2.0
    );
    assert_eq!(
        results[1].data.as_ref().unwrap()["result"]
            .as_f64()
            .unwrap(),
        6.0
    );
}

#[tokio::test]
async fn test_dispatch_atomic_tools_with_provider() {
    let calls = vec![
        tool_call("calculator", serde_json::json!({"expression": "3+3"})),
        tool_call("web_search", serde_json::json!({"query": "test"})),
    ];
    let provider = FakeSearchProvider;
    let results = dispatch_atomic_tools_with_provider(calls, Some(&provider)).await;
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].status, ToolStatus::Ok);
    assert_eq!(results[0].tool, "calculator");
    assert_eq!(results[1].status, ToolStatus::Ok);
    assert_eq!(results[1].tool, "web_search");
}
