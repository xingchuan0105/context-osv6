//! [`Protocol`] implementation for OpenAI chat completions.
use super::request::build_chat_completion_request_body;
use super::types::*;
use crate::protocols::Protocol;
use crate::schema::{FinishReason, LlmError, LlmEvent, LlmRequest, LlmResponse, LlmUsage};

impl Protocol for OpenAiChatProtocol {
    type Body = serde_json::Value;
    type State = OpenAiChatState;

    fn protocol_id(&self) -> &'static str {
        "openai_chat"
    }

    fn build_body(&self, req: &LlmRequest) -> Result<Self::Body, LlmError> {
        Ok(build_chat_completion_request_body(
            &req.config,
            &req.messages,
            req.options.temperature,
            req.options.stream,
            req.options.json_mode,
            req.options.max_tokens,
            &req.tools,
        ))
    }

    fn initial_state(&self, req: &LlmRequest) -> Self::State {
        OpenAiChatState {
            provider: req.config.provider_name(),
            configured_model: req.config.model.clone(),
            model: req.config.model.clone(),
            ..Default::default()
        }
    }

    fn decode_frame(&self, frame: &str) -> Result<serde_json::Value, LlmError> {
        if frame.trim() == "[DONE]" {
            return Ok(serde_json::Value::Null);
        }
        serde_json::from_str(frame).map_err(|error| {
            LlmError::parse(format!(
                "Failed to parse chat completion stream payload: {frame}: {error}"
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

        let chunk: StreamChunk = serde_json::from_value(event.clone())
            .map_err(|error| LlmError::parse(format!("Failed to parse chat completion event: {error}")))?;

        let mut events = Vec::new();

        if let Some(model) = chunk.model {
            state.model = model;
        }

        if let Some(usage) = chunk.usage {
            state.usage = Some(usage.to_llm_usage(state.provider.clone(), state.model.clone()));
        }

        for choice in chunk.choices {
            if let Some(message) = choice.message {
                if let Some(reasoning) = message.reasoning_content.as_deref() {
                    if !reasoning.is_empty() {
                        if !state.reasoning_started {
                            state.reasoning_started = true;
                            events.push(LlmEvent::ReasoningStart {
                                id: REASONING_BLOCK_ID.to_string(),
                            });
                        }
                        state.accumulated_reasoning.push_str(reasoning);
                        events.push(LlmEvent::ReasoningDelta {
                            id: REASONING_BLOCK_ID.to_string(),
                            text: reasoning.to_string(),
                        });
                    }
                }

                if let Some(content) = message.content.as_deref() {
                    if !content.is_empty() {
                        if !state.text_started {
                            state.text_started = true;
                            events.push(LlmEvent::TextStart {
                                id: TEXT_BLOCK_ID.to_string(),
                            });
                        }
                        state.accumulated_content.push_str(content);
                        events.push(LlmEvent::TextDelta {
                            id: TEXT_BLOCK_ID.to_string(),
                            text: content.to_string(),
                        });
                    }
                }

                if let Some(tool_calls) = message.tool_calls.as_deref() {
                    state.tool_calls = map_openai_tool_calls(tool_calls);
                }
                continue;
            }

            let Some(delta) = choice.delta else {
                continue;
            };

            if let Some(reasoning) = delta.reasoning_content.as_deref() {
                if !reasoning.is_empty() {
                    if !state.reasoning_started {
                        state.reasoning_started = true;
                        events.push(LlmEvent::ReasoningStart {
                            id: REASONING_BLOCK_ID.to_string(),
                        });
                    }
                    state.accumulated_reasoning.push_str(reasoning);
                    events.push(LlmEvent::ReasoningDelta {
                        id: REASONING_BLOCK_ID.to_string(),
                        text: reasoning.to_string(),
                    });
                }
            }

            if let Some(content) = delta.content.as_deref() {
                if !content.is_empty() {
                    if !state.text_started {
                        state.text_started = true;
                        events.push(LlmEvent::TextStart {
                            id: TEXT_BLOCK_ID.to_string(),
                        });
                    }
                    state.accumulated_content.push_str(content);
                    events.push(LlmEvent::TextDelta {
                        id: TEXT_BLOCK_ID.to_string(),
                        text: content.to_string(),
                    });
                }
            }
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

        // Tool-only turns (empty content + tool_calls) are valid OpenAI responses.
        let has_tool_calls = state
            .tool_calls
            .as_ref()
            .is_some_and(|calls| !calls.is_empty());
        if state.accumulated_content.is_empty()
            && state.accumulated_reasoning.is_empty()
            && !has_tool_calls
        {
            events.push(LlmEvent::ProviderError {
                message: "Chat completion stream finished without content".to_string(),
                retryable: None,
            });
            return events;
        }

        let usage = state.usage.as_ref().map(usage_to_event_usage);
        events.push(LlmEvent::Finish {
            reason: if has_tool_calls {
                FinishReason::ToolCalls
            } else {
                FinishReason::Stop
            },
            usage,
        });

        events
    }

    fn finalize(&self, state: Self::State) -> Result<LlmResponse, LlmError> {
        let mut content = state.accumulated_content;
        let has_tool_calls = state
            .tool_calls
            .as_ref()
            .is_some_and(|calls| !calls.is_empty());
        if content.is_empty() {
            if !state.accumulated_reasoning.is_empty() {
                content = state.accumulated_reasoning.clone();
            } else if !has_tool_calls {
                return Err(LlmError::EmptyStream);
            }
            // else: tool-only response — keep empty content.
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

