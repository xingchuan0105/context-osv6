use super::*;
use crate::agents::unified::atomic_tools::dispatch_atomic_tool;
use contracts::ToolStatus;

#[tokio::test]
async fn test_weather_query_missing_location() {
    let call = tool_call("weather_query", serde_json::json!({}));
    let result = dispatch_atomic_tool(&call, None).await;
    assert_eq!(result.status, ToolStatus::Error);
    let data = result.data.unwrap();
    assert!(data["error"].as_str().unwrap().contains("missing location"));
}
