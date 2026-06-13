use super::*;
use crate::agents::unified::atomic_tools::dispatch_atomic_tool;
use contracts::ToolStatus;

#[tokio::test]
async fn test_calculator_basic() {
    let call = tool_call("calculator", serde_json::json!({"expression": "1 + 2 * 3"}));
    let result = dispatch_atomic_tool(&call, None).await;
    assert_eq!(result.status, ToolStatus::Ok);
    let data = result.data.unwrap();
    assert_eq!(data["result"].as_f64().unwrap(), 7.0);
}

#[tokio::test]
async fn test_calculator_missing_expression() {
    let call = tool_call("calculator", serde_json::json!({}));
    let result = dispatch_atomic_tool(&call, None).await;
    assert_eq!(result.status, ToolStatus::Error);
    let data = result.data.unwrap();
    assert!(
        data["error"]
            .as_str()
            .unwrap()
            .contains("missing expression")
    );
}

#[tokio::test]
async fn test_calculator_trigonometry() {
    let call = tool_call("calculator", serde_json::json!({"expression": "sin(pi/2)"}));
    let result = dispatch_atomic_tool(&call, None).await;
    assert_eq!(result.status, ToolStatus::Ok);
    let data = result.data.unwrap();
    assert!(data["result"].as_f64().unwrap() > 0.99);
}

#[tokio::test]
async fn test_calculator_division_by_zero() {
    let call = tool_call("calculator", serde_json::json!({"expression": "1/0"}));
    let result = dispatch_atomic_tool(&call, None).await;
    assert_eq!(result.status, ToolStatus::Error);
}

#[tokio::test]
async fn test_calculator_invalid_expression() {
    let call = tool_call("calculator", serde_json::json!({"expression": "1 + * 2"}));
    let result = dispatch_atomic_tool(&call, None).await;
    assert_eq!(result.status, ToolStatus::Error);
}
