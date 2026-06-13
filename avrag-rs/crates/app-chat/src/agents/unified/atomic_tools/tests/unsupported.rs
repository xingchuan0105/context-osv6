use super::*;
use crate::agents::unified::atomic_tools::dispatch_atomic_tool;
use contracts::ToolStatus;

#[tokio::test]
async fn test_unsupported_tool() {
    let call = tool_call("unknown_tool", serde_json::json!({}));
    let result = dispatch_atomic_tool(&call, None).await;
    assert_eq!(result.status, ToolStatus::NotImplemented);
}
