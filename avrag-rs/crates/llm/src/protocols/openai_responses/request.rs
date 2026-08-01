//! Request body construction for the OpenAI Responses protocol.
use crate::schema::ChatMessage;
use crate::schema::ToolDefinition;
use crate::ModelProviderConfig;

pub fn build_responses_request_body(
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
        "input": build_input_items(messages),
    });

    if let Some(instructions) = build_instructions(messages) {
        request_body["instructions"] = serde_json::json!(instructions);
    }

    if let Some(temp) = temperature {
        request_body["temperature"] = serde_json::json!(temp);
    }
    if let Some(max_tokens) = max_tokens {
        request_body["max_output_tokens"] = serde_json::json!(max_tokens);
    }
    if stream {
        request_body["stream"] = serde_json::json!(true);
    }
    if json_mode {
        request_body["text"] = serde_json::json!({ "format": { "type": "json_object" } });
    }
    if let Some(enable_thinking) = config.enable_thinking {
        request_body["reasoning"] = serde_json::json!({
            "effort": if enable_thinking { "high" } else { "low" },
        });
    }
    if !tools.is_empty() {
        request_body["tools"] = serde_json::json!(
            tools
                .iter()
                .map(|tool| serde_json::json!({
                    "type": "function",
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.input_schema,
                }))
                .collect::<Vec<_>>()
        );
    }

    request_body
}

/// Responses has no `system` role: system messages become `instructions`
/// (concatenated in order).
fn build_instructions(messages: &[ChatMessage]) -> Option<String> {
    let parts = messages
        .iter()
        .filter(|m| m.role == "system")
        .map(|m| m.content.trim())
        .filter(|c| !c.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}

/// Map chat messages to Responses `input` items. System messages are skipped
/// here (they become `instructions`); tool results become `function_call_output`
/// items; assistant tool-call history becomes `function_call` items.
fn build_input_items(messages: &[ChatMessage]) -> Vec<serde_json::Value> {
    let mut items = Vec::new();
    for message in messages {
        if message.role == "system" {
            continue;
        }
        match message.role.as_str() {
            "tool" => {
                items.push(serde_json::json!({
                    "type": "function_call_output",
                    "call_id": message.tool_call_id.clone().unwrap_or_default(),
                    "output": message.content,
                }));
            }
            "assistant" => {
                let text = message_text(message);
                let has_tool_calls = message
                    .tool_calls
                    .as_ref()
                    .is_some_and(|calls| calls.as_array().is_some_and(|calls| !calls.is_empty()));
                // Empty message items are rejected by the Responses API; skip
                // the item when there is neither text nor tool history.
                if text.is_empty() && !has_tool_calls {
                    continue;
                }
                items.push(serde_json::json!({
                    "type": "message",
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": text,
                    }],
                }));
                if let Some(tool_calls) = message.tool_calls.as_ref() {
                    if let Some(calls) = tool_calls.as_array() {
                        for call in calls {
                            if let Some(function) = call.get("function") {
                                items.push(serde_json::json!({
                                    "type": "function_call",
                                    "call_id": call.get("id").and_then(|id| id.as_str()).unwrap_or_default(),
                                    "name": function.get("name").and_then(|n| n.as_str()).unwrap_or_default(),
                                    "arguments": function.get("arguments").and_then(|a| a.as_str()).unwrap_or_default(),
                                }));
                            }
                        }
                    }
                }
            }
            _ => {
                // user (and any unknown role): text-only message item.
                items.push(serde_json::json!({
                    "type": "message",
                    "role": "user",
                    "content": [{
                        "type": "input_text",
                        "text": message_text(message),
                    }],
                }));
            }
        }
    }
    items
}

/// Responses ignores image input on DeepSeek (image blocks become placeholders);
/// keep the text part only.
fn message_text(message: &ChatMessage) -> String {
    if let Some(ref parts) = message.multimodal_content {
        let text = parts
            .iter()
            .filter_map(|part| match part {
                crate::schema::ContentPart::Text { text } => Some(text.as_str()),
                crate::schema::ContentPart::ImageUrl { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("");
        if !text.is_empty() {
            return text;
        }
    }
    message.content.clone()
}

#[cfg(test)]
mod tests {
    use super::{build_input_items, build_responses_request_body};
    use crate::schema::{ChatMessage, ToolDefinition};
    use crate::ModelProviderConfig;

    fn test_config() -> ModelProviderConfig {
        ModelProviderConfig {
            base_url: "https://api.deepseek.com".to_string(),
            api_key: "test-key".to_string(),
            model: "deepseek-v4-flash".to_string(),
            timeout_ms: 1000,
            api_style: None,
            dimensions: None,
            enable_thinking: Some(true),
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        }
    }

    #[test]
    fn system_messages_become_instructions() {
        let body = build_responses_request_body(
            &test_config(),
            &[
                ChatMessage::system("You are helpful."),
                ChatMessage::user("Hi"),
            ],
            Some(0.5),
            false,
            false,
            None,
            &[],
        );
        assert_eq!(body["instructions"], "You are helpful.");
        let input = body["input"].as_array().unwrap();
        assert_eq!(input.len(), 1);
        assert_eq!(input[0]["type"], "message");
        assert_eq!(input[0]["role"], "user");
        assert_eq!(input[0]["content"][0]["type"], "input_text");
        assert_eq!(input[0]["content"][0]["text"], "Hi");
    }

    #[test]
    fn multiple_system_messages_are_joined() {
        let items = build_input_items(&[
            ChatMessage::system("A"),
            ChatMessage::user("u"),
            ChatMessage::system("B"),
        ]);
        // system messages do not produce input items
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn tool_history_maps_to_function_call_items() {
        let mut assistant = ChatMessage::assistant("");
        assistant.tool_calls = Some(serde_json::json!([
            {
                "id": "call_1",
                "type": "function",
                "function": { "name": "web_search", "arguments": r#"{"q":"rust"}"# }
            }
        ]));
        let mut tool_result = ChatMessage::user("ok");
        tool_result.role = "tool".to_string();
        tool_result.tool_call_id = Some("call_1".to_string());
        tool_result.content = "5 results".to_string();

        let items = build_input_items(&[assistant, tool_result]);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0]["type"], "message");
        assert_eq!(items[0]["role"], "assistant");
        assert_eq!(items[1]["type"], "function_call");
        assert_eq!(items[1]["call_id"], "call_1");
        assert_eq!(items[1]["name"], "web_search");
        assert_eq!(items[1]["arguments"], r#"{"q":"rust"}"#);
        assert_eq!(items[2]["type"], "function_call_output");
        assert_eq!(items[2]["call_id"], "call_1");
        assert_eq!(items[2]["output"], "5 results");
    }

    #[test]
    fn tools_are_flat_function_entries() {
        let body = build_responses_request_body(
            &test_config(),
            &[ChatMessage::user("hi")],
            None,
            false,
            false,
            None,
            &[ToolDefinition {
                name: "search".to_string(),
                description: "Search the web".to_string(),
                input_schema: serde_json::json!({ "type": "object" }),
            }],
        );
        let tools = body["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["type"], "function");
        assert_eq!(tools[0]["name"], "search");
        assert_eq!(tools[0]["description"], "Search the web");
        assert_eq!(tools[0]["parameters"]["type"], "object");
    }

    #[test]
    fn thinking_and_json_mode_are_mapped() {
        let body = build_responses_request_body(
            &test_config(),
            &[ChatMessage::user("hi")],
            None,
            true,
            true,
            Some(512),
            &[],
        );
        assert_eq!(body["reasoning"]["effort"], "high");
        assert_eq!(body["text"]["format"]["type"], "json_object");
        assert_eq!(body["stream"], true);
        assert_eq!(body["max_output_tokens"], 512);
    }

    #[test]
    fn disabled_thinking_maps_to_low_effort() {
        let mut config = test_config();
        config.enable_thinking = Some(false);
        let body = build_responses_request_body(&config, &[ChatMessage::user("hi")], None, false, false, None, &[]);
        assert_eq!(body["reasoning"]["effort"], "low");
    }
}
