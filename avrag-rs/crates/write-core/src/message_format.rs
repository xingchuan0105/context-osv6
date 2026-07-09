//! Chat message builders for WriteRefine tool-use history (provider-shaped).

use avrag_llm::ChatMessage;
use contracts::{ToolCall, ToolResult};

/// Assistant turn carrying one or more tool calls (OpenAI function-call shape).
pub fn build_assistant_message_with_tool_calls(
    calls: &[ToolCall],
    call_ids: &[String],
    content: &str,
    reasoning_content: Option<String>,
) -> ChatMessage {
    let openai_calls: Vec<serde_json::Value> = calls
        .iter()
        .zip(call_ids.iter())
        .map(|(call, id)| {
            serde_json::json!({
                "id": id,
                "type": "function",
                "function": {
                    "name": call.tool,
                    "arguments": serde_json::to_string(&call.args)
                        .unwrap_or_else(|_| "{}".to_string()),
                }
            })
        })
        .collect();

    ChatMessage {
        role: "assistant".to_string(),
        content: content.to_string(),
        multimodal_content: None,
        name: None,
        tool_call_id: None,
        tool_calls: Some(serde_json::json!(openai_calls)),
        reasoning_content,
    }
}

/// Tool-role message with serialized ToolResult body.
pub fn build_tool_message(call_id: &str, tool_name: &str, result: &ToolResult) -> ChatMessage {
    let body = serde_json::json!({
        "tool": tool_name,
        "status": result.status,
        "data": result.data,
    });
    ChatMessage {
        role: "tool".to_string(),
        content: body.to_string(),
        multimodal_content: None,
        name: Some(tool_name.to_string()),
        tool_call_id: Some(call_id.to_string()),
        tool_calls: None,
        reasoning_content: None,
    }
}
