use super::Protocol;
use crate::schema::{
    FinishReason, LlmError, LlmEvent, LlmRequest, LlmResponse, LlmUsage, Usage,
};
use serde::Deserialize;

const TEXT_BLOCK_ID: &str = "text-0";
const REASONING_BLOCK_ID: &str = "reasoning-0";

#[derive(Debug, Clone, Copy, Default)]
pub struct AnthropicMessagesProtocol;

#[derive(Debug, Default)]
pub struct AnthropicMessagesState {
    accumulated_content: String,
    accumulated_reasoning: String,
    usage: Option<LlmUsage>,
    model: String,
    provider: String,
    #[allow(dead_code)]
    configured_model: String,
    tool_calls: Option<Vec<contracts::ToolCall>>,
    text_started: bool,
    reasoning_started: bool,
}

pub fn build_anthropic_messages_body(
    req: &LlmRequest,
) -> Result<serde_json::Value, LlmError> {
    let mut system_parts = Vec::new();
    let mut messages = Vec::new();

    for message in &req.messages {
        if message.role == "system" {
            system_parts.push(build_text_block(&message.content, req.config.enable_cache));
            continue;
        }

        let role = match message.role.as_str() {
            "assistant" => "assistant",
            "tool" => "user",
            _ => "user",
        };

        let mut content_blocks = Vec::new();

        if message.role == "tool" {
            content_blocks.push(serde_json::json!({
                "type": "tool_result",
                "tool_use_id": message.tool_call_id.clone().unwrap_or_default(),
                "content": message.content,
            }));
        } else if let Some(ref parts) = message.multimodal_content {
            for part in parts {
                match part {
                    crate::schema::ContentPart::Text { text } => {
                        content_blocks.push(build_text_block(text, None));
                    }
                    crate::schema::ContentPart::ImageUrl { image_url } => {
                        content_blocks.push(serde_json::json!({
                            "type": "image",
                            "source": {
                                "type": "url",
                                "url": image_url.url,
                            }
                        }));
                    }
                }
            }
        } else {
            if let Some(ref reasoning) = message.reasoning_content {
                if !reasoning.is_empty() {
                    content_blocks.push(serde_json::json!({
                        "type": "thinking",
                        "thinking": reasoning,
                    }));
                }
            }
            if !message.content.is_empty() {
                content_blocks.push(build_text_block(&message.content, None));
            }
            if let Some(ref tool_calls) = message.tool_calls {
                if let Some(array) = tool_calls.as_array() {
                    for call in array {
                        if let Some(id) = call.get("id").and_then(|v| v.as_str()) {
                            let name = call
                                .get("function")
                                .and_then(|f| f.get("name"))
                                .and_then(|v| v.as_str())
                                .or_else(|| call.get("name").and_then(|v| v.as_str()))
                                .unwrap_or("");
                            let input = call
                                .get("function")
                                .and_then(|f| f.get("arguments"))
                                .cloned()
                                .unwrap_or_else(|| call.get("input").cloned().unwrap_or(serde_json::json!({})));
                            let parsed_input = if input.is_string() {
                                serde_json::from_str(input.as_str().unwrap_or("{}"))
                                    .unwrap_or(serde_json::json!({}))
                            } else {
                                input
                            };
                            content_blocks.push(serde_json::json!({
                                "type": "tool_use",
                                "id": id,
                                "name": name,
                                "input": parsed_input,
                            }));
                        }
                    }
                }
            }
        }

        if content_blocks.is_empty() {
            content_blocks.push(build_text_block("", None));
        }

        messages.push(serde_json::json!({
            "role": role,
            "content": content_blocks,
        }));
    }

    let max_tokens = req.options.max_tokens.unwrap_or(4096);
    let mut body = serde_json::json!({
        "model": req.config.model,
        "max_tokens": max_tokens,
        "messages": messages,
    });

    if !system_parts.is_empty() {
        body["system"] = if system_parts.len() == 1 && req.config.enable_cache != Some(true) {
            system_parts[0]["text"].clone()
        } else {
            serde_json::Value::Array(system_parts)
        };
    }

    if let Some(temp) = req.options.temperature {
        body["temperature"] = serde_json::json!(temp);
    }
    if req.options.stream {
        body["stream"] = serde_json::json!(true);
    }
    if !req.tools.is_empty() {
        body["tools"] = serde_json::json!(
            req.tools
                .iter()
                .map(|tool| serde_json::json!({
                    "name": tool.name,
                    "description": tool.description,
                    "input_schema": tool.input_schema,
                }))
                .collect::<Vec<_>>()
        );
    }

    Ok(body)
}

fn build_text_block(text: &str, enable_cache: Option<bool>) -> serde_json::Value {
    let mut block = serde_json::json!({
        "type": "text",
        "text": text,
    });
    if enable_cache == Some(true) {
        block["cache_control"] = serde_json::json!({ "type": "ephemeral" });
    }
    block
}

#[derive(Debug, Deserialize, Default)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
    #[serde(default)]
    cache_read_input_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct ContentBlockDelta {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    thinking: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamEvent {
    #[serde(default)]
    r#type: Option<String>,
    #[serde(default)]
    delta: Option<ContentBlockDelta>,
    #[serde(default)]
    usage: Option<AnthropicUsage>,
    #[serde(default)]
    message: Option<serde_json::Value>,
}

fn usage_from_anthropic(usage: &AnthropicUsage, provider: &str, model: &str) -> LlmUsage {
    LlmUsage {
        prompt_tokens: usage.input_tokens,
        completion_tokens: usage.output_tokens,
        total_tokens: usage.input_tokens + usage.output_tokens,
        provider: provider.to_string(),
        model: model.to_string(),
        cached_tokens: usage.cache_read_input_tokens,
    }
}

fn usage_to_event_usage(usage: &LlmUsage) -> Usage {
    Usage {
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
        cached_tokens: usage.cached_tokens,
    }
}

fn extract_non_stream_content(value: &serde_json::Value) -> (String, Option<String>, Option<Vec<contracts::ToolCall>>) {
    let mut content = String::new();
    let mut reasoning = String::new();
    let mut tool_calls = Vec::new();

    if let Some(blocks) = value.get("content").and_then(|v| v.as_array()) {
        for block in blocks {
            match block.get("type").and_then(|v| v.as_str()) {
                Some("text") => {
                    if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                        content.push_str(text);
                    }
                }
                Some("thinking") => {
                    if let Some(text) = block.get("thinking").and_then(|v| v.as_str()) {
                        reasoning.push_str(text);
                    }
                }
                Some("tool_use") => {
                    let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let input = block.get("input").cloned().unwrap_or(serde_json::json!({}));
                    tool_calls.push(contracts::ToolCall {
                        tool: name.to_string(),
                        version: "1.0".to_string(),
                        args: input,
                    });
                }
                _ => {}
            }
        }
    }

    let reasoning_content = if reasoning.is_empty() {
        None
    } else {
        Some(reasoning)
    };
    let tool_calls = if tool_calls.is_empty() {
        None
    } else {
        Some(tool_calls)
    };
    (content, reasoning_content, tool_calls)
}

impl Protocol for AnthropicMessagesProtocol {
    type Body = serde_json::Value;
    type State = AnthropicMessagesState;

    fn protocol_id(&self) -> &'static str {
        "anthropic_messages"
    }

    fn build_body(&self, req: &LlmRequest) -> Result<Self::Body, LlmError> {
        build_anthropic_messages_body(req)
    }

    fn initial_state(&self, req: &LlmRequest) -> Self::State {
        AnthropicMessagesState {
            provider: req.config.provider_name(),
            configured_model: req.config.model.clone(),
            model: req.config.model.clone(),
            ..Default::default()
        }
    }

    fn decode_frame(&self, frame: &str) -> Result<serde_json::Value, LlmError> {
        if frame.trim().is_empty() || frame.trim() == "[DONE]" {
            return Ok(serde_json::Value::Null);
        }
        serde_json::from_str(frame).map_err(|error| {
            LlmError::parse(format!(
                "Failed to parse Anthropic stream payload: {frame}: {error}"
            ))
        })
    }

    fn step(
        &self,
        state: &mut Self::State,
        event: &serde_json::Value,
    ) -> Result<Vec<LlmEvent>, LlmError> {
        if event.is_null() {
            return Ok(Vec::new());
        }

        // Non-streaming full response
        if event.get("content").is_some() && event.get("type").is_none() {
            let (content, reasoning, tool_calls) = extract_non_stream_content(event);
            state.accumulated_content = content;
            state.accumulated_reasoning = reasoning.unwrap_or_default();
            state.tool_calls = tool_calls;
            if let Some(model) = event.get("model").and_then(|v| v.as_str()) {
                state.model = model.to_string();
            }
            if let Some(usage) = event.get("usage") {
                let usage: AnthropicUsage = serde_json::from_value(usage.clone())
                    .map_err(|e| LlmError::parse(format!("invalid Anthropic usage: {e}")))?;
                state.usage = Some(usage_from_anthropic(
                    &usage,
                    &state.provider,
                    &state.model,
                ));
            }
            return Ok(Vec::new());
        }

        let stream_event: StreamEvent = serde_json::from_value(event.clone())
            .map_err(|error| LlmError::parse(format!("Failed to parse Anthropic event: {error}")))?;
        let mut events = Vec::new();

        if let Some(message) = stream_event.message {
            if let Some(model) = message.get("model").and_then(|v| v.as_str()) {
                state.model = model.to_string();
            }
            if let Some(usage) = message.get("usage") {
                let usage: AnthropicUsage = serde_json::from_value(usage.clone())
                    .map_err(|e| LlmError::parse(format!("invalid Anthropic usage: {e}")))?;
                state.usage = Some(usage_from_anthropic(
                    &usage,
                    &state.provider,
                    &state.model,
                ));
            }
        }

        if let Some(usage) = stream_event.usage {
            state.usage = Some(usage_from_anthropic(
                &usage,
                &state.provider,
                &state.model,
            ));
        }

        match stream_event.r#type.as_deref() {
            Some("content_block_delta") => {
                if let Some(delta) = stream_event.delta {
                    if let Some(text) = delta.text.as_deref() {
                        if !text.is_empty() {
                            if !state.text_started {
                                state.text_started = true;
                                events.push(LlmEvent::TextStart {
                                    id: TEXT_BLOCK_ID.to_string(),
                                });
                            }
                            state.accumulated_content.push_str(text);
                            events.push(LlmEvent::TextDelta {
                                id: TEXT_BLOCK_ID.to_string(),
                                text: text.to_string(),
                            });
                        }
                    }
                    if let Some(thinking) = delta.thinking.as_deref() {
                        if !thinking.is_empty() {
                            if !state.reasoning_started {
                                state.reasoning_started = true;
                                events.push(LlmEvent::ReasoningStart {
                                    id: REASONING_BLOCK_ID.to_string(),
                                });
                            }
                            state.accumulated_reasoning.push_str(thinking);
                            events.push(LlmEvent::ReasoningDelta {
                                id: REASONING_BLOCK_ID.to_string(),
                                text: thinking.to_string(),
                            });
                        }
                    }
                }
            }
            Some("message_stop") | Some("message_delta") => {}
            _ => {}
        }

        Ok(events)
    }

    fn on_halt(&self, state: &Self::State) -> Vec<LlmEvent> {
        let mut events = Vec::new();

        if state.text_started {
            events.push(LlmEvent::TextEnd {
                id: TEXT_BLOCK_ID.to_string(),
            });
        }
        if state.reasoning_started {
            events.push(LlmEvent::ReasoningEnd {
                id: REASONING_BLOCK_ID.to_string(),
            });
        }

        if state.accumulated_content.is_empty() && state.accumulated_reasoning.is_empty() {
            events.push(LlmEvent::ProviderError {
                message: "Anthropic stream finished without content".to_string(),
                retryable: None,
            });
            return events;
        }

        let usage = state.usage.as_ref().map(usage_to_event_usage);
        events.push(LlmEvent::Finish {
            reason: FinishReason::Stop,
            usage,
        });

        events
    }

    fn finalize(&self, state: Self::State) -> Result<LlmResponse, LlmError> {
        let mut content = state.accumulated_content;
        if content.is_empty() {
            if state.accumulated_reasoning.is_empty() {
                return Err(LlmError::EmptyStream);
            }
            content = state.accumulated_reasoning.clone();
        }

        let reasoning_content = if state.accumulated_reasoning.is_empty() {
            None
        } else {
            Some(state.accumulated_reasoning)
        };

        Ok(LlmResponse {
            content,
            reasoning_content,
            usage: state.usage.unwrap_or_else(|| LlmUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
                provider: state.provider,
                model: state.model.clone(),
                cached_tokens: 0,
            }),
            model: state.model,
            tool_calls: state.tool_calls,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{build_anthropic_messages_body, AnthropicMessagesProtocol};
    use crate::schema::{ChatMessage, GenerationOptions, LlmRequest};
    use crate::ModelProviderConfig;
    use crate::protocols::Protocol;

    fn test_config() -> ModelProviderConfig {
        ModelProviderConfig {
            base_url: "https://api.anthropic.com/v1".to_string(),
            api_key: "test-key".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
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
    fn anthropic_body_extracts_system_and_sets_cache_control() {
        let req = LlmRequest::new(
            vec![
                ChatMessage::system("You are helpful."),
                ChatMessage::user("Hello"),
            ],
            test_config(),
        );
        let body = build_anthropic_messages_body(&req).unwrap();
        assert_eq!(body["model"], "claude-sonnet-4-20250514");
        assert_eq!(body["messages"][0]["role"], "user");
        let system = &body["system"];
        assert_eq!(system[0]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn anthropic_non_stream_response_is_finalized() {
        let protocol = AnthropicMessagesProtocol;
        let req = LlmRequest::new(vec![ChatMessage::user("hi")], test_config());
        let mut state = protocol.initial_state(&req);
        let response = serde_json::json!({
            "model": "claude-sonnet-4-20250514",
            "content": [{"type": "text", "text": "Hello there"}],
            "usage": {"input_tokens": 5, "output_tokens": 3}
        });
        protocol.step(&mut state, &response).unwrap();
        let finalized = protocol.finalize(state).unwrap();
        assert_eq!(finalized.content, "Hello there");
        assert_eq!(finalized.usage.prompt_tokens, 5);
        assert_eq!(finalized.usage.completion_tokens, 3);
    }

    #[test]
    fn anthropic_stream_delta_emits_text_events() {
        let protocol = AnthropicMessagesProtocol;
        let req = LlmRequest::new(vec![ChatMessage::user("hi")], test_config())
            .with_options(GenerationOptions {
                stream: true,
                ..Default::default()
            });
        let mut state = protocol.initial_state(&req);
        let event = serde_json::json!({
            "type": "content_block_delta",
            "index": 0,
            "delta": {"type": "text_delta", "text": "Hi"}
        });
        let events = protocol.step(&mut state, &event).unwrap();
        assert!(events.iter().any(|e| matches!(e, crate::schema::LlmEvent::TextDelta { .. })));
        assert_eq!(state.accumulated_content, "Hi");
    }
}
