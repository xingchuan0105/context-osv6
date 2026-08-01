use avrag_llm::ChatMessage;

/// Safely truncate a string to at most `max_chars` characters (not bytes).
pub(crate) fn truncate_preview(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect::<String>() + "..."
    }
}

/// Truncate tool/sandbox observation text to a char budget, appending a marker if truncated.
/// Used to bound the size of untrusted content re-injected into the LLM context.
pub(crate) fn truncate_observation(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let truncated: String = text.chars().take(max_chars).collect();
    let original = text.chars().count();
    format!(
        "{truncated}
...[truncated, {original} chars total]"
    )
}

/// Per-tool-message char budget for `data` payloads re-injected into the
/// LLM context (roughly 6-8k tokens worth of evidence).
pub(crate) const TOOL_MESSAGE_MAX_CHARS: usize = 24_000;

/// Budget-aware JSON shrinker: keeps the JSON structurally valid while
/// trimming array lengths and long strings so the serialized form stays
/// within roughly `budget_chars`. Used to bound unbounded tool results
/// (e.g. RAG retrieval chunks) that would otherwise blow the context window.
pub(crate) fn trim_json_for_context(
    value: &serde_json::Value,
    budget_chars: usize,
) -> serde_json::Value {
    if serde_json::to_string(value).map(|s| s.len()).unwrap_or(0) <= budget_chars {
        return value.clone();
    }
    trim_json_inner(value, budget_chars)
}

fn trim_json_inner(value: &serde_json::Value, budget_chars: usize) -> serde_json::Value {
    match value {
        serde_json::Value::Array(items) => {
            let mut out = Vec::new();
            let mut used = 2usize; // `[]`
            for item in items {
                let trimmed = trim_json_inner(item, budget_chars.saturating_sub(used + 24).max(8));
                let len = serde_json::to_string(&trimmed)
                    .map(|s| s.len())
                    .unwrap_or(0);
                let sep = if out.is_empty() { 0 } else { 2 }; // `, `
                if used + len + sep > budget_chars {
                    // Keep at least the first entry (already trimmed) so the
                    // payload is never reduced to an empty container.
                    if out.is_empty() {
                        out.push(trimmed);
                    }
                    break;
                }
                out.push(trimmed);
                used += len + sep;
            }
            serde_json::Value::Array(out)
        }
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::new();
            let mut used = 2usize; // `{}`
            // Track the largest trimmed field seen so far; when the budget is
            // exceeded and nothing was kept yet, fall back to that field
            // (content fields like `chunks`/`results` are typically the
            // largest) instead of the lexicographically first key.
            let mut fallback: Option<(String, serde_json::Value)> = None;
            let mut fallback_len = 0usize;
            for (key, item) in map {
                let trimmed = trim_json_inner(item, budget_chars.saturating_sub(used + 24).max(8));
                let mut entry_map = serde_json::Map::new();
                entry_map.insert(key.clone(), trimmed.clone());
                let len = serde_json::to_string(&serde_json::Value::Object(entry_map))
                    .map(|s| s.len())
                    .unwrap_or(0);
                let sep = if out.is_empty() { 0 } else { 2 };
                if len > fallback_len {
                    fallback = Some((key.clone(), trimmed.clone()));
                    fallback_len = len;
                }
                if used + len + sep > budget_chars {
                    // Never drop the largest field entirely.
                    if out.is_empty() {
                        if let Some((fallback_key, fallback_value)) = fallback {
                            out.insert(fallback_key, fallback_value);
                        }
                    }
                    break;
                }
                out.insert(key.clone(), trimmed);
                used += len + sep;
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::String(s) => {
            // Budget is measured in *bytes* (serde_json::to_string().len()),
            // so truncate by byte length at a char boundary — never by char
            // count, which would overshoot the budget 3-4x on CJK text.
            if s.len() <= budget_chars {
                value.clone()
            } else {
                // Reserve room for the JSON quotes, the `[truncated]` marker
                // and one level of enclosing object/array syntax.
                let keep_bytes = budget_chars.saturating_sub(24);
                let end = s.floor_char_boundary(keep_bytes);
                serde_json::Value::String(format!("{}...[truncated]", &s[..end]))
            }
        }
        _ => value.clone(),
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
///
/// The `data` payload is budget-trimmed (structurally valid JSON) so that
/// unbounded retrieval results cannot blow the context window.
pub(crate) fn build_tool_message(
    call_id: &str,
    tool_name: &str,
    result: &contracts::ToolResult,
) -> ChatMessage {
    let data = result
        .data
        .as_ref()
        .map(|d| trim_json_for_context(d, TOOL_MESSAGE_MAX_CHARS));
    let body = serde_json::json!({
        "tool": tool_name,
        "status": result.status,
        "data": data,
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

    #[test]
    fn trim_json_keeps_structure_within_budget() {
        use super::trim_json_for_context;

        // Small payload passes through untouched.
        let small = serde_json::json!({"a": "short"});
        assert_eq!(trim_json_for_context(&small, 10_000), small);

        // Large array is trimmed structurally (JSON stays parseable).
        let big = serde_json::json!({
            "chunks": (0..50).map(|i| {
                serde_json::json!({"id": format!("chunk-{i}"), "text": "x".repeat(200)})
            }).collect::<Vec<_>>()
        });
        let trimmed = trim_json_for_context(&big, 2_000);
        let parsed: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&trimmed).unwrap()).unwrap();
        assert!(parsed.is_object());
        let chunks = parsed["chunks"].as_array().unwrap();
        assert!(
            chunks.len() < 50,
            "array should be trimmed, got {}",
            chunks.len()
        );
        assert!(chunks.len() >= 1);
        // Serialized form respects the budget.
        assert!(serde_json::to_string(&trimmed).unwrap().len() <= 2_000 + 64);

        // Oversized string is truncated with a marker.
        let long = serde_json::json!({"text": "y".repeat(5_000)});
        let trimmed = trim_json_for_context(&long, 500);
        let text = trimmed["text"].as_str().unwrap();
        assert!(text.len() < 5_000);
        assert!(text.ends_with("...[truncated]"));
    }

    #[test]
    fn tool_message_trims_oversized_data() {
        use super::build_tool_message;
        let result = contracts::ToolResult {
            tool: "dense_retrieval".to_string(),
            version: "1".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({
                "chunks": (0..100).map(|i| {
                    serde_json::json!({"id": format!("chunk-{i}"), "text": "z".repeat(1_000)})
                }).collect::<Vec<_>>()
            })),
            trace: None,
        };
        let msg = build_tool_message("call_0", "dense_retrieval", &result);
        assert!(
            msg.content.len() < 40_000,
            "data should be trimmed, got {}",
            msg.content.len()
        );
        // Still valid JSON inside the message content.
        let parsed: serde_json::Value = serde_json::from_str(&msg.content).unwrap();
        assert_eq!(parsed["tool"], "dense_retrieval");
        let chunks = parsed["data"]["chunks"].as_array().unwrap();
        assert!(chunks.len() < 100);
    }
}
