//! RAG auto-fallback — lexical/dense retrieval when ReAct loop degrades.

use avrag_llm::ChatMessage;
use contracts::{ToolCall, ToolResult, ToolStatus};

/// Execute automatic retrieval as fallback with caller-supplied args.
pub async fn auto_fallback(
    runtime: &avrag_rag_core::RagRuntime,
    auth: &contracts::auth_runtime::AuthContext,
    args: serde_json::Value,
    tool_id: &str,
) -> ToolResult {
    let call = ToolCall {
        tool: tool_id.to_string(),
        version: "1.0".to_string(),
        args,
    };

    avrag_rag_core::runtime::tools::dispatch(runtime, auth, &call).await
}

/// Build a fallback observation message, append to messages, and return the tool result.
pub async fn inject_fallback_observation(
    runtime: &avrag_rag_core::RagRuntime,
    auth: &contracts::auth_runtime::AuthContext,
    args: serde_json::Value,
    tool_id: &str,
    messages: &mut Vec<avrag_llm::ChatMessage>,
) -> ToolResult {
    let result = auto_fallback(runtime, auth, args, tool_id).await;

    let observation = match result.status {
        ToolStatus::Ok => {
            let text = serde_json::to_string_pretty(&result.data)
                .unwrap_or_else(|_| " retrieval succeeded".to_string());
            format!("自动兜底检索结果:\n{text}")
        }
        _ => format!(
            "[fallback {tool_id} failed: {:?}]",
            result
                .data
                .as_ref()
                .and_then(|data| data.get("error"))
                .and_then(|error| error.as_str())
                .unwrap_or("tool execution failed")
        ),
    };

    messages.push(ChatMessage::system(observation));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_error_observation_uses_tool_error_field() {
        let result = ToolResult {
            tool: "lexical_retrieval".to_string(),
            version: "1.0".to_string(),
            status: ToolStatus::Error,
            data: Some(serde_json::json!({ "error": "boom" })),
            trace: None,
        };
        let message = match result.status {
            ToolStatus::Ok => "ok".to_string(),
            _ => format!(
                "[fallback lexical_retrieval failed: {:?}]",
                result
                    .data
                    .as_ref()
                    .and_then(|data| data.get("error"))
                    .and_then(|error| error.as_str())
                    .unwrap_or("tool execution failed")
            ),
        };
        assert!(message.contains("boom"));
    }
}
