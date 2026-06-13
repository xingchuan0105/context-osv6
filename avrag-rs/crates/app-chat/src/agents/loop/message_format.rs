use avrag_llm::ChatMessage;

/// Safely truncate a string to at most `max_chars` characters (not bytes).
pub(crate) fn truncate_preview(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect::<String>() + "..."
    }
}

/// Build an OpenAI-format `assistant` message carrying `tool_calls`.
/// `call_ids` must be parallel to `calls` (e.g. `call_0`, `call_1`, ...).
/// If the LLM also emitted reasoning text in `content`, it is preserved so
/// the next iteration can see the model's chain-of-thought.
pub(crate) fn build_assistant_message_with_tool_calls(
    calls: &[contracts::ToolCall],
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

/// Build a `tool` role message from a native tool result, keyed by the
/// synthetic call id used in the assistant message.
pub(crate) fn build_tool_message(
    call_id: &str,
    tool_name: &str,
    result: &contracts::ToolResult,
) -> ChatMessage {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assistant_tool_calls_use_openai_format() {
        let calls = vec![contracts::ToolCall {
            tool: "dense_retrieval".to_string(),
            version: "1".to_string(),
            args: serde_json::json!({"query": "rust"}),
        }];
        let call_ids = vec!["call_0".to_string()];
        let msg = build_assistant_message_with_tool_calls(
            &calls,
            &call_ids,
            "thinking...",
            Some("internal reasoning".to_string()),
        );

        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content, "thinking...");
        assert_eq!(msg.reasoning_content.as_deref(), Some("internal reasoning"));
        let tc = msg.tool_calls.unwrap();
        let arr = tc.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["id"], "call_0");
        assert_eq!(arr[0]["type"], "function");
        assert_eq!(arr[0]["function"]["name"], "dense_retrieval");
        assert_eq!(
            arr[0]["function"]["arguments"].as_str().unwrap(),
            r#"{"query":"rust"}"#
        );
    }

    #[test]
    fn tool_message_carries_matching_call_id() {
        let result = contracts::ToolResult {
            tool: "web_search".to_string(),
            version: "1".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({"hits": 3})),
            trace: None,
        };
        let msg = build_tool_message("call_1", "web_search", &result);

        assert_eq!(msg.role, "tool");
        assert_eq!(msg.tool_call_id.as_deref(), Some("call_1"));
        assert_eq!(msg.name.as_deref(), Some("web_search"));
        assert!(msg.content.contains("\"hits\":3"));
    }
}
