//! [`Protocol`] implementation for the OpenAI Responses protocol
//! (DeepSeek `/v1/responses`, OpenAI-compatible).
use super::request::build_responses_request_body;
use super::types::*;
use crate::protocols::Protocol;
use crate::schema::{FinishReason, LlmError, LlmEvent, LlmRequest, LlmResponse, LlmUsage};

const EVENT_TEXT_DELTA: &str = "response.output_text.delta";
const EVENT_REASONING_DELTA: &str = "response.reasoning_text.delta";
const EVENT_OUTPUT_ITEM_ADDED: &str = "response.output_item.added";
const EVENT_FUNCTION_ARGS_DELTA: &str = "response.function_call_arguments.delta";
const EVENT_FUNCTION_ARGS_DONE: &str = "response.function_call_arguments.done";
const EVENT_COMPLETED: &str = "response.completed";
const EVENT_INCOMPLETE: &str = "response.incomplete";
const EVENT_FAILED: &str = "response.failed";

impl Protocol for OpenAiResponsesProtocol {
    type Body = serde_json::Value;
    type State = OpenAiResponsesState;

    fn protocol_id(&self) -> &'static str {
        "openai_responses"
    }

    fn build_body(&self, req: &LlmRequest) -> Result<Self::Body, LlmError> {
        Ok(build_responses_request_body(
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
        OpenAiResponsesState {
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
                "Failed to parse responses stream payload: {frame}: {error}"
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

        let event_type = event.get("type").and_then(|t| t.as_str());
        let Some(event_type) = event_type else {
            // Non-streaming path: the full Responses object.
            return apply_responses_object(state, event);
        };

        let mut events = Vec::new();
        match event_type {
            EVENT_TEXT_DELTA => {
                let text = event.get("delta").and_then(|d| d.as_str()).unwrap_or("");
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
            EVENT_REASONING_DELTA => {
                let text = event.get("delta").and_then(|d| d.as_str()).unwrap_or("");
                if !text.is_empty() {
                    if !state.reasoning_started {
                        state.reasoning_started = true;
                        events.push(LlmEvent::ReasoningStart {
                            id: REASONING_BLOCK_ID.to_string(),
                        });
                    }
                    state.accumulated_reasoning.push_str(text);
                    events.push(LlmEvent::ReasoningDelta {
                        id: REASONING_BLOCK_ID.to_string(),
                        text: text.to_string(),
                    });
                }
            }
            EVENT_OUTPUT_ITEM_ADDED => {
                let Some(item) = event.get("item") else {
                    return Ok(events);
                };
                if item.get("type").and_then(|t| t.as_str()) == Some("function_call") {
                    let index = event
                        .get("output_index")
                        .and_then(|i| i.as_u64())
                        .unwrap_or(0) as usize;
                    ensure_tool_slot(state, index);
                    state.tool_accumulators[index] = ToolCallAcc {
                        call_id: item
                            .get("call_id")
                            .and_then(|c| c.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        name: item
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        arguments: String::new(),
                    };
                }
            }
            EVENT_FUNCTION_ARGS_DELTA => {
                let delta = event.get("delta").and_then(|d| d.as_str()).unwrap_or("");
                if !delta.is_empty() {
                    let index = event
                        .get("output_index")
                        .and_then(|i| i.as_u64())
                        .unwrap_or(0) as usize;
                    ensure_tool_slot(state, index);
                    state.tool_accumulators[index].arguments.push_str(delta);
                }
            }
            EVENT_FUNCTION_ARGS_DONE => {
                // The done event may carry the full arguments; fall back to it
                // when no delta arrived (single-shot non-delta providers).
                if let Some(arguments) = event.get("arguments").and_then(|a| a.as_str()) {
                    let index = event
                        .get("output_index")
                        .and_then(|i| i.as_u64())
                        .unwrap_or(0) as usize;
                    ensure_tool_slot(state, index);
                    if state.tool_accumulators[index].arguments.is_empty() {
                        state.tool_accumulators[index].arguments = arguments.to_string();
                    }
                }
            }
            EVENT_COMPLETED | EVENT_INCOMPLETE => {
                if let Some(obj) = event.get("response") {
                    events.extend(apply_responses_object(state, obj)?);
                }
                if event_type == EVENT_INCOMPLETE {
                    state.incomplete = true;
                }
            }
            EVENT_FAILED => {
                if let Some(obj) = event.get("response") {
                    events.extend(apply_responses_object(state, obj)?);
                }
                if state.failed_message.is_none() {
                    state.failed_message = event
                        .get("response")
                        .and_then(|r| r.get("error"))
                        .and_then(|e| e.get("message"))
                        .and_then(|m| m.as_str())
                        .map(String::from);
                }
            }
            _ => {
                // response.created / in_progress / output_item.done /
                // content_part.* / web_search_call.* — nothing to accumulate.
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

        if let Some(message) = &state.failed_message {
            events.push(LlmEvent::ProviderError {
                message: message.clone(),
                retryable: None,
            });
            return events;
        }

        let has_tool_calls = state
            .tool_accumulators
            .iter()
            .any(|acc| !acc.arguments.is_empty());
        if state.accumulated_content.is_empty()
            && state.accumulated_reasoning.is_empty()
            && !has_tool_calls
        {
            events.push(LlmEvent::ProviderError {
                message: "Responses stream finished without content".to_string(),
                retryable: None,
            });
            return events;
        }

        let reason = if has_tool_calls {
            FinishReason::ToolCalls
        } else if state.incomplete {
            FinishReason::Length
        } else {
            FinishReason::Stop
        };
        let usage = state.usage.as_ref().map(usage_to_event_usage);
        events.push(LlmEvent::Finish { reason, usage });
        events
    }

    fn finalize(&self, mut state: Self::State) -> Result<LlmResponse, LlmError> {
        flush_tool_calls(&mut state);

        if let Some(message) = &state.failed_message {
            return Err(LlmError::protocol(message.clone()));
        }

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
                reasoning_tokens: 0,
            }),
            model: state.model,
            tool_calls: state.tool_calls,
        })
    }
}

/// Absorb a full Responses object (non-streaming response body, or the
/// `response` payload of a terminal stream event).
fn apply_responses_object(
    state: &mut OpenAiResponsesState,
    value: &serde_json::Value,
) -> Result<Vec<LlmEvent>, LlmError> {
    let obj: ResponsesObject = serde_json::from_value(value.clone()).map_err(|error| {
        LlmError::parse(format!(
            "Failed to parse responses object: {value}: {error}"
        ))
    })?;

    let mut events = Vec::new();
    if let Some(model) = obj.model {
        state.model = model;
    }
    if let Some(usage) = obj.usage {
        state.usage = Some(usage.to_llm_usage(state.provider.clone(), state.model.clone()));
    }
    match obj.status.as_deref() {
        Some("failed") => {
            state.failed_message = obj.error.and_then(|error| error.message);
        }
        Some("incomplete") => {
            state.incomplete = true;
        }
        _ => {}
    }
    absorb_output_items(state, &obj.output, &mut events);
    Ok(events)
}

/// Fold `output` items into the state. In streaming mode the text/reasoning
/// deltas already arrived, so only the function calls (and anything the
/// terminal event carries that deltas did not) are absorbed.
fn absorb_output_items(
    state: &mut OpenAiResponsesState,
    items: &[ResponsesOutputItem],
    events: &mut Vec<LlmEvent>,
) {
    for item in items {
        match item.item_type.as_str() {
            "message" => {
                let text = item
                    .content
                    .as_ref()
                    .map(|parts| {
                        parts
                            .iter()
                            .filter(|part| part.part_type == "output_text")
                            .filter_map(|part| part.text.clone())
                            .collect::<Vec<_>>()
                            .join("")
                    })
                    .unwrap_or_default();
                if !state.text_started && !text.is_empty() {
                    state.text_started = true;
                    events.push(LlmEvent::TextStart {
                        id: TEXT_BLOCK_ID.to_string(),
                    });
                    state.accumulated_content.push_str(&text);
                    events.push(LlmEvent::TextDelta {
                        id: TEXT_BLOCK_ID.to_string(),
                        text,
                    });
                }
            }
            "reasoning" => {
                // DeepSeek returns reasoning text in `content` blocks typed
                // `reasoning_text`; OpenAI uses `summary` blocks typed
                // `summary_text`. Accept both so the parser is provider-neutral.
                let text = reasoning_text_of(item);
                if !state.reasoning_started && !text.is_empty() {
                    state.reasoning_started = true;
                    events.push(LlmEvent::ReasoningStart {
                        id: REASONING_BLOCK_ID.to_string(),
                    });
                    state.accumulated_reasoning.push_str(&text);
                    events.push(LlmEvent::ReasoningDelta {
                        id: REASONING_BLOCK_ID.to_string(),
                        text,
                    });
                }
            }
            "function_call" => {
                let call_id = item.call_id.clone().unwrap_or_default();
                let name = item.name.clone().unwrap_or_default();
                let Some(arguments) = item.arguments.clone() else {
                    continue;
                };
                // 1) Already accumulated via stream deltas (call_id known).
                if state
                    .tool_accumulators
                    .iter()
                    .any(|acc| acc.call_id == call_id && !acc.arguments.is_empty())
                {
                    continue;
                }
                // 2) Same call_id, but the slot is still empty (delta never
                //    arrived, or single-shot non-delta provider) — backfill.
                if let Some(existing) = state
                    .tool_accumulators
                    .iter_mut()
                    .find(|acc| acc.call_id == call_id)
                {
                    if existing.arguments.is_empty() {
                        existing.arguments = arguments;
                    }
                    continue;
                }
                // 3) A delta-accumulated slot whose `output_item.added` never
                //    arrived (call_id/name still empty) — backfill identity
                //    instead of pushing a duplicate tool call.
                if let Some(existing) = state.tool_accumulators.iter_mut().find(|acc| {
                    acc.call_id.is_empty()
                        && !acc.arguments.is_empty()
                        && acc.arguments == arguments
                }) {
                    existing.call_id = call_id;
                    existing.name = name;
                    continue;
                }
                state.tool_accumulators.push(ToolCallAcc {
                    call_id,
                    name,
                    arguments,
                });
            }
            _ => {}
        }
    }
}

fn ensure_tool_slot(state: &mut OpenAiResponsesState, index: usize) {
    while state.tool_accumulators.len() <= index {
        state.tool_accumulators.push(ToolCallAcc::default());
    }
}

/// Extract reasoning text from a `reasoning` output item, accepting both the
/// DeepSeek shape (`content: [{type: "reasoning_text", text}]`) and the
/// OpenAI shape (`summary: [{type: "summary_text", text}]`).
fn reasoning_text_of(item: &ResponsesOutputItem) -> String {
    let mut text = String::new();
    if let Some(parts) = item.content.as_ref() {
        for part in parts {
            if part.part_type == "reasoning_text" {
                if let Some(value) = part.text.as_ref() {
                    text.push_str(value);
                }
            }
        }
    }
    if text.is_empty() {
        if let Some(parts) = item.summary.as_ref() {
            for part in parts {
                if part.part_type == "summary_text" {
                    if let Some(value) = part.text.as_ref() {
                        text.push_str(value);
                    }
                }
            }
        }
    }
    text
}

fn flush_tool_calls(state: &mut OpenAiResponsesState) {
    let calls = state
        .tool_accumulators
        .iter()
        .filter(|acc| !acc.arguments.is_empty())
        .map(|acc| contracts::ToolCall {
            tool: acc.name.clone(),
            version: "1.0".to_string(),
            args: serde_json::from_str(&acc.arguments)
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
        })
        .collect::<Vec<_>>();
    if !calls.is_empty() {
        state.tool_calls = Some(calls);
    }
}
