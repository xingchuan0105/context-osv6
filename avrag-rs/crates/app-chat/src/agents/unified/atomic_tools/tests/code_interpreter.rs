use super::*;
use crate::agents::unified::atomic_tools::dispatch_atomic_tool;
use contracts::ToolStatus;

#[tokio::test]
async fn test_code_interpreter_simple() {
    let call = tool_call(
        "code_interpreter",
        serde_json::json!({"code": "print(1 + 2)"}),
    );
    let result = dispatch_atomic_tool(&call, None).await;
    assert_eq!(result.status, ToolStatus::Ok);
    let data = result.data.unwrap();
    assert!(data["stdout"].as_str().unwrap().contains("3"));
    assert!(data["success"].as_bool().unwrap());
}

#[tokio::test]
async fn test_code_interpreter_missing_code() {
    let call = tool_call("code_interpreter", serde_json::json!({}));
    let result = dispatch_atomic_tool(&call, None).await;
    assert_eq!(result.status, ToolStatus::Error);
    let data = result.data.unwrap();
    assert!(data["error"].as_str().unwrap().contains("missing code"));
}

#[tokio::test]
async fn test_code_interpreter_stderr() {
    let call = tool_call(
        "code_interpreter",
        serde_json::json!({"code": "raise ValueError('error')"}),
    );
    let result = dispatch_atomic_tool(&call, None).await;
    assert_eq!(result.status, ToolStatus::Ok);
    let data = result.data.unwrap();
    assert!(data["stderr"].as_str().unwrap().contains("ValueError"));
    assert!(data["success"].as_bool().unwrap());
}

#[tokio::test]
async fn test_code_interpreter_exception() {
    let call = tool_call("code_interpreter", serde_json::json!({"code": "1/0"}));
    let result = dispatch_atomic_tool(&call, None).await;
    assert_eq!(result.status, ToolStatus::Ok);
    let data = result.data.unwrap();
    assert!(
        data["stderr"]
            .as_str()
            .unwrap()
            .contains("ZeroDivisionError")
    );
    assert!(data["success"].as_bool().unwrap());
}

#[tokio::test]
async fn test_code_interpreter_result_field() {
    let call = tool_call("code_interpreter", serde_json::json!({"code": "x = 42"}));
    let result = dispatch_atomic_tool(&call, None).await;
    assert_eq!(result.status, ToolStatus::Ok);
    let data = result.data.unwrap();
    assert!(data["result"].is_null() || data["result"] == serde_json::Value::Null);
}
