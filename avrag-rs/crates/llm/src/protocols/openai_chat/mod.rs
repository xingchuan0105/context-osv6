//! OpenAI Chat Completions protocol (request + stream + Protocol impl).
mod protocol;
mod request;
mod stream;
mod types;

// Protocol impl is attached via the trait; keep the marker type public.
pub use request::build_chat_completion_request_body;
pub use types::{OpenAiChatProtocol, OpenAiChatState};

// Re-export parser / wire helpers for in-crate legacy callers (client stream path).
pub(crate) use stream::ChatCompletionStreamParser;
pub(crate) use types::ApiUsageRaw;

#[cfg(test)]
mod tests {
    use super::request::build_chat_completion_request_body;
    use super::stream::ChatCompletionStreamParser;
    use super::types::ApiUsageRaw;
    use crate::schema::ChatMessage;
    use crate::ModelProviderConfig;

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
            &[],
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
            &[],
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
            &[],
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
            &[],
        );

        assert_eq!(body["enable_thinking"], false);
        assert!(body.get("thinking").is_none());
    }

    #[test]
    fn request_includes_prompt_cache_when_enable_cache_is_true() {
        let mut config = test_config("https://api.deepseek.com", None);
        config.enable_cache = Some(true);
        let body = build_chat_completion_request_body(
            &config,
            &[ChatMessage::user("hello")],
            None,
            false,
            false,
            None,
            &[],
        );
        assert_eq!(body["prompt_cache"], true);
    }

    #[test]
    fn request_omits_prompt_cache_when_enable_cache_is_none() {
        let config = test_config("https://api.deepseek.com", None);
        let body = build_chat_completion_request_body(
            &config,
            &[ChatMessage::user("hello")],
            None,
            false,
            false,
            None,
            &[],
        );
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

        let body = build_chat_completion_request_body(
            &config,
            &[msg1, msg2, msg3],
            None,
            false,
            false,
            None,
            &[],
        );

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
            &[],
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
            &[],
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
            &[],
        );
        assert_eq!(body["response_format"]["type"], "json_object");
    }

    #[test]
    fn json_mode_omitted_for_non_deepseek_providers() {
        let config = test_config("https://dashscope.aliyuncs.com/compatible-mode/v1", None);
        let body = build_chat_completion_request_body(
            &config,
            &[ChatMessage::user("return json")],
            None,
            false,
            true,
            None,
            &[],
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
            &[],
        );
        assert!(body.get("response_format").is_none());
    }

    #[test]
    fn chat_completion_stream_parser_accumulates_delta_content_and_usage() {
        let mut parser =
            ChatCompletionStreamParser::new("openai".to_string(), "gpt-test".to_string());
        let mut observed = String::new();

        parser
            .feed_chunk(
                br#"data: {"id":"chatcmpl-1","model":"gpt-stream","choices":[{"delta":{"content":"Hel"}}]}

"#,
                &mut |delta| observed.push_str(delta),
                &mut |_| {},
            )
            .unwrap();
        parser
            .feed_chunk(
                br#"data: {"choices":[{"delta":{"content":"lo"}}]}

data: {"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":3,"total_tokens":15}}

data: [DONE]

"#,
                &mut |delta| observed.push_str(delta),
                &mut |_| {},
            )
            .unwrap();

        let response = parser
            .finish(&mut |delta| observed.push_str(delta), &mut |_| {})
            .unwrap();

        assert_eq!(observed, "Hello");
        assert_eq!(response.content, "Hello");
        assert_eq!(response.model, "gpt-stream");
        assert_eq!(response.usage.prompt_tokens, 12);
        assert_eq!(response.usage.completion_tokens, 3);
        assert_eq!(response.usage.total_tokens, 15);
        assert_eq!(response.usage.provider, "openai");
    }

    #[test]
    fn chat_completion_stream_parser_handles_chunked_lines() {
        let mut parser =
            ChatCompletionStreamParser::new("openai".to_string(), "gpt-test".to_string());
        let mut observed = String::new();

        parser
            .feed_chunk(
                br#"data: {"choices":[{"delta":{"content":"A"#,
                &mut |delta| observed.push_str(delta),
                &mut |_| {},
            )
            .unwrap();
        parser
            .feed_chunk(
                br#"B"}}]}

data: [DONE]

"#,
                &mut |delta| observed.push_str(delta),
                &mut |_| {},
            )
            .unwrap();

        let response = parser
            .finish(&mut |delta| observed.push_str(delta), &mut |_| {})
            .unwrap();
        assert_eq!(observed, "AB");
        assert_eq!(response.content, "AB");
    }

    #[test]
    fn chat_completion_stream_parser_rejects_empty_content_stream() {
        let mut parser =
            ChatCompletionStreamParser::new("openai".to_string(), "gpt-test".to_string());

        parser
            .feed_chunk(
                br#"data: {"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":0,"total_tokens":12}}

data: [DONE]

"#,
                &mut |_delta| {},
                &mut |_| {},
            )
            .unwrap();

        let error = parser.finish(&mut |_delta| {}, &mut |_| {}).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("Chat completion stream finished without content")
        );
    }

    #[test]
    fn chat_completion_stream_parser_falls_back_to_reasoning_when_content_empty() {
        let mut parser =
            ChatCompletionStreamParser::new("deepseek".to_string(), "deepseek-chat".to_string());

        let mut reasoning_observed = String::new();
        parser
            .feed_chunk(
                br#"data: {"choices":[{"delta":{"reasoning_content":"Final answer from reasoning."}}]}

data: [DONE]

"#,
                &mut |_delta| {},
                &mut |delta| reasoning_observed.push_str(delta),
            )
            .unwrap();

        let response = parser
            .finish(&mut |_delta| {}, &mut |delta| {
                reasoning_observed.push_str(delta)
            })
            .unwrap();
        assert_eq!(reasoning_observed, "Final answer from reasoning.");
        assert_eq!(response.content, "Final answer from reasoning.");
        assert_eq!(
            response.reasoning_content.as_deref(),
            Some("Final answer from reasoning.")
        );
    }

    #[test]
    fn usage_parses_cached_tokens_from_provider_fields() {
        let raw: ApiUsageRaw = serde_json::from_str(
            r#"{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15,"prompt_cache_hit_tokens":8}"#,
        )
        .unwrap();
        assert_eq!(raw.cached_token_count(), 8);

        let raw2: ApiUsageRaw = serde_json::from_str(
            r#"{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15,"prompt_tokens_details":{"cached_tokens":3}}"#,
        )
        .unwrap();
        assert_eq!(raw2.cached_token_count(), 3);

        let usage = raw.to_llm_usage("deepseek".to_string(), "model".to_string());
        assert_eq!(usage.cached_tokens, 8);
    }
}
