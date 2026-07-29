//! RAG auto-fallback — lexical/dense retrieval when ReAct loop degrades.
//!
//! Runtime dispatch is via [`super::deps::LoopRuntimeDeps`] (Wave B1 follow-up);
//! this module only formats observations.

use avrag_llm::ChatMessage;
use contracts::{ToolResult, ToolStatus};

/// Build a fallback observation message, append to messages, and return the tool result.
pub fn append_fallback_observation(
    tool_id: &str,
    result: ToolResult,
    messages: &mut Vec<ChatMessage>,
) -> ToolResult {
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
        let mut messages = Vec::new();
        let out = append_fallback_observation("lexical_retrieval", result, &mut messages);
        assert_eq!(out.status, ToolStatus::Error);
        assert!(messages[0].content.contains("boom"));
    }
}
