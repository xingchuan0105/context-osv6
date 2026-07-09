//! Request body construction for OpenAI chat completions.
use crate::schema::{ChatMessage, ToolDefinition};
use crate::ModelProviderConfig;

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
    if let Some(enable_thinking) = config.enable_thinking {
        let base = config.base_url.to_ascii_lowercase();
        if base.contains("deepseek") {
            let mut thinking = serde_json::json!({
                "type": if enable_thinking { "enabled" } else { "disabled" },
            });
            if enable_thinking {
                thinking["reasoning_effort"] = serde_json::json!("max");
            }
            request_body["thinking"] = thinking;
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
    if config.enable_cache == Some(true) {
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

