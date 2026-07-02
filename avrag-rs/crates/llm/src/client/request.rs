use super::ChatMessage;
use crate::ModelProviderConfig;

pub(super) fn build_chat_completion_request_body(
    config: &ModelProviderConfig,
    messages: &[ChatMessage],
    temperature: Option<f32>,
    stream: bool,
    json_mode: bool,
    max_tokens: Option<u32>,
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

    // JSON Output (`response_format: json_object`) constrains the model to emit
    // a valid JSON string, eliminating the code-block-on-synthesis failure mode
    // at the API layer. Gated to OpenAI-compatible providers that honor this
    // field (DeepSeek, SiliconFlow). The prompt must already contain "json" +
    // a format example. See https://api-docs.deepseek.com/zh-cn/guides/json_mode
    // and https://api-docs.siliconflow.cn/docs/api/chat-completions-post.
    if json_mode {
        let base = config.base_url.to_ascii_lowercase();
        if base.contains("deepseek") || base.contains("siliconflow") {
            request_body["response_format"] = serde_json::json!({ "type": "json_object" });
        }
    }

    request_body
}

#[cfg(test)]
mod tests {
    use super::build_chat_completion_request_body;
    use crate::{ChatMessage, ModelProviderConfig};

    fn test_config(base_url: &str, enable_thinking: Option<bool>) -> ModelProviderConfig {
        ModelProviderConfig {
            base_url: base_url.to_string(),
            api_key: "test-key".to_string(),
            model: "test-model".to_string(),
            timeout_ms: 1000,
            api_style: None,
            dimensions: None,
            enable_thinking,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        }
    }

    #[test]
    fn deepseek_request_maps_enable_thinking_to_thinking_object() {
        let config = test_config("https://api.deepseek.com", Some(false));
        let body = build_chat_completion_request_body(
            &config,
            &[ChatMessage::user("hello")],
            Some(0.3),
            false,
            false,
            None,
        );

        assert_eq!(body["thinking"]["type"], "disabled");
        assert!(body.get("enable_thinking").is_none());
    }

    #[test]
    fn deepseek_request_uses_max_reasoning_effort_when_thinking_enabled() {
        let config = test_config("https://api.deepseek.com", Some(true));
        let body = build_chat_completion_request_body(
            &config,
            &[ChatMessage::user("hello")],
            Some(0.3),
            false,
            false,
            None,
        );

        assert_eq!(body["thinking"]["type"], "enabled");
        assert_eq!(body["thinking"]["reasoning_effort"], "max");
    }

    #[test]
    fn gemini_request_omits_enable_thinking_field() {
        let config = test_config(
            "https://generativelanguage.googleapis.com/v1beta/openai",
            Some(false),
        );
        let body = build_chat_completion_request_body(
            &config,
            &[ChatMessage::user("hello")],
            Some(0.3),
            false,
            false,
            None,
        );

        assert!(body.get("enable_thinking").is_none());
        assert!(body.get("thinking").is_none());
    }

    #[test]
    fn non_deepseek_request_keeps_enable_thinking_field() {
        let config = test_config(
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            Some(false),
        );
        let body = build_chat_completion_request_body(
            &config,
            &[ChatMessage::user("hello")],
            Some(0.3),
            false,
            false,
            None,
        );

        assert_eq!(body["enable_thinking"], false);
        assert!(body.get("thinking").is_none());
    }

    #[test]
    fn request_includes_prompt_cache_when_enable_cache_is_true() {
        let mut config = test_config("https://api.deepseek.com", None);
        config.enable_cache = Some(true);
        let body =
            build_chat_completion_request_body(&config, &[ChatMessage::user("hello")], None, false, false, None);
        assert_eq!(body["prompt_cache"], true);
    }

    #[test]
    fn request_omits_prompt_cache_when_enable_cache_is_none() {
        let config = test_config("https://api.deepseek.com", None);
        let body =
            build_chat_completion_request_body(&config, &[ChatMessage::user("hello")], None, false, false, None);
        assert!(body.get("prompt_cache").is_none());
    }

    #[test]
    fn request_serializes_message_tool_fields() {
        let config = test_config("https://api.openai.com", None);
        let mut msg1 = ChatMessage::user("Hello");
        msg1.name = Some("user_alice".to_string());

        let mut msg2 = ChatMessage::assistant("");
        msg2.tool_calls = Some(serde_json::json!([{
            "id": "call_123",
            "type": "function",
            "function": {
                "name": "test_tool",
                "arguments": "{}"
            }
        }]));

        let msg3 = ChatMessage {
            role: "tool".to_string(),
            content: "success".to_string(),
            multimodal_content: None,
            name: Some("test_tool".to_string()),
            tool_call_id: Some("call_123".to_string()),
            tool_calls: None,
            reasoning_content: None,
        };

        let body = build_chat_completion_request_body(&config, &[msg1, msg2, msg3], None, false, false, None);

        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 3);

        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[0]["content"], "Hello");
        assert_eq!(messages[0]["name"], "user_alice");

        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[1]["content"], "");
        assert_eq!(messages[1]["tool_calls"][0]["id"], "call_123");

        assert_eq!(messages[2]["role"], "tool");
        assert_eq!(messages[2]["content"], "success");
        assert_eq!(messages[2]["name"], "test_tool");
        assert_eq!(messages[2]["tool_call_id"], "call_123");
    }

    #[test]
    fn request_serializes_assistant_reasoning_content_for_thinking_mode() {
        let config = test_config("https://api.deepseek.com", Some(true));
        let assistant = ChatMessage {
            role: "assistant".to_string(),
            content: String::new(),
            multimodal_content: None,
            name: None,
            tool_call_id: None,
            tool_calls: Some(serde_json::json!([{
                "id": "call_0",
                "type": "function",
                "function": {
                    "name": "dense_retrieval",
                    "arguments": r#"{"query":"rust"}"#
                }
            }])),
            reasoning_content: Some("Let me search the knowledge base.".to_string()),
        };

        let body = build_chat_completion_request_body(
            &config,
            &[ChatMessage::user("hello"), assistant],
            None,
            false,
            false,
            None,
        );

        let messages = body["messages"].as_array().unwrap();
        assert_eq!(
            messages[1]["reasoning_content"],
            "Let me search the knowledge base."
        );
    }

    #[test]
    fn deepseek_json_mode_sets_response_format() {
        let config = test_config("https://api.deepseek.com", None);
        let body = build_chat_completion_request_body(
            &config,
            &[ChatMessage::user("return json")],
            None,
            false,
            true,
            None,
        );
        assert_eq!(body["response_format"]["type"], "json_object");
    }

    #[test]
    fn siliconflow_json_mode_sets_response_format() {
        let config = test_config("https://api.siliconflow.cn/v1", None);
        let body = build_chat_completion_request_body(
            &config,
            &[ChatMessage::user("return json")],
            None,
            false,
            true,
            None,
        );
        assert_eq!(body["response_format"]["type"], "json_object");
    }

    #[test]
    fn json_mode_omitted_for_non_deepseek_providers() {
        let config =
            test_config("https://dashscope.aliyuncs.com/compatible-mode/v1", None);
        let body = build_chat_completion_request_body(
            &config,
            &[ChatMessage::user("return json")],
            None,
            false,
            true,
            None,
        );
        assert!(body.get("response_format").is_none());
    }

    #[test]
    fn json_mode_omitted_when_not_requested() {
        let config = test_config("https://api.deepseek.com", None);
        let body = build_chat_completion_request_body(
            &config,
            &[ChatMessage::user("hello")],
            None,
            false,
            false,
            None,
        );
        assert!(body.get("response_format").is_none());
    }
}
