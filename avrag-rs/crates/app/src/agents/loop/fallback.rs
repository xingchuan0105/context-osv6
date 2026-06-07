//! RAG auto-fallback — dense retrieval when ReAct loop degrades.

use avrag_llm::ChatMessage;
use common::{AppError, ToolCall, ToolStatus};

/// Execute automatic retrieval as fallback with caller-supplied args.
pub async fn auto_fallback(
    runtime: &avrag_rag_core::RagRuntime,
    auth: &avrag_auth::AuthContext,
    args: serde_json::Value,
    tool_id: &str,
) -> Result<String, AppError> {
    let call = ToolCall {
        tool: tool_id.to_string(),
        version: "1.0".to_string(),
        args,
    };

    let result = avrag_rag_core::runtime::tools::dispatch(runtime, auth, &call).await;

    match result.status {
        ToolStatus::Ok => {
            let text = serde_json::to_string_pretty(&result.data)
                .unwrap_or_else(|_| " retrieval succeeded".to_string());
            Ok(format!("自动兜底检索结果:\n{text}"))
        }
        _ => Err(AppError::internal(format!(
            "fallback {tool_id} failed: {:?}",
            result.data
        ))),
    }
}

/// Build a fallback observation message and append to messages.
pub async fn inject_fallback_observation(
    runtime: &avrag_rag_core::RagRuntime,
    auth: &avrag_auth::AuthContext,
    args: serde_json::Value,
    tool_id: &str,
    messages: &mut Vec<avrag_llm::ChatMessage>,
) -> Result<(), AppError> {
    let observation = auto_fallback(runtime, auth, args, tool_id).await?;
    messages.push(ChatMessage::system(observation));
    Ok(())
}
