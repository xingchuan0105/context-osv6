//! Request body construction for OpenAI chat completions.
use crate::ModelProviderConfig;
use crate::schema::{ChatMessage, ToolDefinition};

pub fn build_chat_completion_request_body(
    config: &ModelProviderConfig,
    messages: &[ChatMessage],
    temperature: Option<f32>,
    stream: bool,
    json_mode: bool,
    max_tokens: Option<u32>,
    tools: &[ToolDefinition],
) -> serde_json::Value {
    let mut request_body = serde_json::json!({
        "model": config.model,
        "messages": messages
            .iter()
            .map(|m| {
                let mut msg = serde_json::json!({ "role": m.role });
                if let Some(ref parts) = m.multimodal_content {
                    msg["content"] = serde_json::to_value(parts).unwrap_or_default();
                } else {
                    msg["content"] = serde_json::json!(m.content);
                }
                if let Some(ref name) = m.name {
                    msg["name"] = serde_json::json!(name);
                }
                if let Some(ref tool_call_id) = m.tool_call_id {
                    msg["tool_call_id"] = serde_json::json!(tool_call_id);
                }
                if let Some(ref tool_calls) = m.tool_calls {
                    msg["tool_calls"] = tool_calls.clone();
                }
                if let Some(ref reasoning_content) = m.reasoning_content {
                    msg["reasoning_content"] = serde_json::json!(reasoning_content);
                }
                msg
            })
            .collect::<Vec<_>>(),
    });

    if let Some(temp) = temperature {
        request_body["temperature"] = serde_json::json!(temp);
    }
    if let Some(max_tokens) = max_tokens {
        request_body["max_tokens"] = serde_json::json!(max_tokens);
    }
    let base = config.base_url.to_ascii_lowercase();
    // DashScope explicit context cache (ephemeral, 5-min TTL): mark the system
    // prefix and the last message so the shared system prompt hits across
    // questions and the growing SaC conversation hits across rounds.
    if config.enable_cache == Some(true) && base.contains("dashscope") {
        inject_dashscope_cache_markers(&mut request_body);
    }
    if let Some(enable_thinking) = config.enable_thinking {
        if base.contains("deepseek") {
            let mut thinking = serde_json::json!({
                "type": if enable_thinking { "enabled" } else { "disabled" },
            });
            if enable_thinking {
                thinking["reasoning_effort"] = serde_json::json!("max");
            }
            request_body["thinking"] = thinking;
        } else if base.contains("wafer") {
            // Wafer: top-level reasoning_effort (none/low/medium/high/max).
            request_body["reasoning_effort"] =
                serde_json::json!(if enable_thinking { "max" } else { "none" });
        } else if base.contains("generativelanguage") || base.contains("googleapis.com") {
            // Gemini OpenAI-compat rejects unknown `enable_thinking` (400 INVALID_ARGUMENT).
        } else {
            request_body["enable_thinking"] = serde_json::json!(enable_thinking);
        }
    }
    if stream {
        request_body["stream"] = serde_json::json!(true);
        request_body["stream_options"] = serde_json::json!({
            "include_usage": true,
        });
    }
    // DeepSeek-style prompt_cache flag (deepseek/siliconflow only; DashScope's
    // cache rides cache_control markers above, Wafer reports cache natively).
    if config.enable_cache == Some(true)
        && (base.contains("deepseek") || base.contains("siliconflow"))
    {
        request_body["prompt_cache"] = serde_json::json!(true);
    }

    if json_mode {
        let base = config.base_url.to_ascii_lowercase();
        if base.contains("deepseek") || base.contains("siliconflow") {
            request_body["response_format"] = serde_json::json!({ "type": "json_object" });
        }
    }

    if !tools.is_empty() {
        let openai_tools = tools
            .iter()
            .map(|tool| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.input_schema,
                    }
                })
            })
            .collect::<Vec<_>>();
        request_body["tools"] = serde_json::json!(openai_tools);
    }

    request_body
}

/// Mark the system prefix and the last message with DashScope ephemeral
/// `cache_control`. Content must be in array-of-parts form to carry the
/// marker; string content is converted, existing multimodal parts get the
/// marker on their last part.
fn inject_dashscope_cache_markers(request_body: &mut serde_json::Value) {
    fn mark(m: &mut serde_json::Value) {
        if let Some(text) = m["content"].as_str() {
            let text = text.to_string();
            m["content"] = serde_json::json!([{
                "type": "text",
                "text": text,
                "cache_control": {"type": "ephemeral"},
            }]);
        } else if let Some(parts) = m["content"].as_array_mut() {
            if let Some(last) = parts.last_mut() {
                last["cache_control"] = serde_json::json!({"type": "ephemeral"});
            }
        }
    }
    let Some(msgs) = request_body["messages"].as_array_mut() else {
        return;
    };
    // The system prompt is the largest prefix shared across questions.
    if let Some(sys) = msgs.iter_mut().find(|m| m["role"] == "system") {
        mark(sys);
    }
    // The tail marker extends/refreshes the cache across SaC rounds (5-min TTL
    // resets on hit; creation is only for the newly appended portion).
    if let Some(last) = msgs.last_mut() {
        mark(last);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::ChatMessage;

    fn cfg(base_url: &str) -> ModelProviderConfig {
        ModelProviderConfig {
            base_url: base_url.into(),
            api_key: "k".into(),
            model: "m".into(),
            timeout_ms: 1000,
            api_style: None,
            dimensions: None,
            enable_thinking: None,
            enable_cache: Some(true),
            rpm_limit: None,
            tpm_limit: None,
        }
    }

    #[test]
    fn dashscope_cache_marks_system_and_last_message() {
        let messages = vec![
            ChatMessage::system("sys"),
            ChatMessage::user("u1"),
            ChatMessage::user("u2"),
        ];
        let body = build_chat_completion_request_body(
            &cfg("https://dashscope.aliyuncs.com/compatible-mode/v1"),
            &messages,
            None,
            false,
            false,
            None,
            &[],
        );
        let msgs = body["messages"].as_array().unwrap();
        assert!(msgs[0]["content"][0]["cache_control"]["type"] == "ephemeral");
        assert!(msgs[1]["content"].is_string(), "middle message untouched");
        assert!(msgs[2]["content"][0]["cache_control"]["type"] == "ephemeral");
        assert!(body.get("prompt_cache").is_none(), "no deepseek flag on dashscope");
    }

    #[test]
    fn deepseek_keeps_prompt_cache_flag_and_no_markers() {
        let messages = vec![ChatMessage::system("sys"), ChatMessage::user("u")];
        let body = build_chat_completion_request_body(
            &cfg("https://api.deepseek.com"),
            &messages,
            None,
            false,
            false,
            None,
            &[],
        );
        assert_eq!(body["prompt_cache"], true);
        assert!(body["messages"][0]["content"].is_string());
    }
}
