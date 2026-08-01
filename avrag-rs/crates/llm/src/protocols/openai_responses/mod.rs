//! OpenAI Responses protocol (DeepSeek `/v1/responses`, OpenAI-compatible).
//! Request + stream + Protocol impl.
mod protocol;
mod request;
mod types;

// Protocol impl is attached via the trait; keep the marker type public.
pub use request::build_responses_request_body;
pub use types::{OpenAiResponsesProtocol, OpenAiResponsesState};

#[cfg(test)]
mod tests {
    use super::types::OpenAiResponsesProtocol;
    use crate::protocols::Protocol;
    use crate::schema::{FinishReason, LlmEvent, LlmRequest};
    use crate::ModelProviderConfig;
    use serde_json::json;

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

    fn request() -> LlmRequest {
        LlmRequest::new(vec![crate::schema::ChatMessage::user("hi")], test_config())
    }

    fn text_deltas(events: &[LlmEvent]) -> Vec<String> {
        events
            .iter()
            .filter_map(|e| match e {
                LlmEvent::TextDelta { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn non_streaming_response_absorbs_text_usage_and_tool_call() {
        let protocol = OpenAiResponsesProtocol;
        let mut state = protocol.initial_state(&request());

        let body = json!({
            "id": "resp_1",
            "model": "deepseek-v4-flash",
            "status": "completed",
            "output": [
                {
                    "type": "message",
                    "id": "msg_1",
                    "role": "assistant",
                    "status": "completed",
                    "content": [{ "type": "output_text", "text": "Found results", "annotations": [] }]
                },
                {
                    "type": "function_call",
                    "id": "fc_1",
                    "call_id": "call_7",
                    "name": "web_search",
                    "arguments": r#"{"q":"rust"}"#
                }
            ],
            "usage": {
                "input_tokens": 42,
                "output_tokens": 17,
                "total_tokens": 59,
                "input_tokens_details": { "cached_tokens": 30 },
                "output_tokens_details": { "reasoning_tokens": 5 }
            }
        });

        let events = protocol.step(&mut state, &body).unwrap();
        assert_eq!(text_deltas(&events), vec!["Found results".to_string()]);

        let halt = protocol.on_halt(&state);
        assert!(halt.iter().any(|e| matches!(e, LlmEvent::Finish { reason: FinishReason::ToolCalls, .. })));

        let response = protocol.finalize(state).unwrap();
        assert_eq!(response.content, "Found results");
        assert_eq!(response.usage.prompt_tokens, 42);
        assert_eq!(response.usage.completion_tokens, 17);
        assert_eq!(response.usage.cached_tokens, 30);
        let calls = response.tool_calls.unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "web_search");
        assert_eq!(calls[0].args["q"], "rust");
    }

    #[test]
    fn streaming_text_delta_events_feed_deltas_and_finish() {
        let protocol = OpenAiResponsesProtocol;
        let mut state = protocol.initial_state(&request());

        let mut events = protocol
            .step(&mut state, &json!({"type": "response.created", "response": {"id": "r1"}}))
            .unwrap();
        events.extend(
            protocol
                .step(&mut state, &json!({"type": "response.output_text.delta", "delta": "Hel", "output_index": 0}))
                .unwrap(),
        );
        events.extend(
            protocol
                .step(&mut state, &json!({"type": "response.output_text.delta", "delta": "lo", "output_index": 0}))
                .unwrap(),
        );
        assert_eq!(text_deltas(&events), vec!["Hel".to_string(), "lo".to_string()]);

        events.extend(
            protocol
                .step(
                    &mut state,
                    &json!({
                        "type": "response.completed",
                        "response": {
                            "id": "r1",
                            "model": "deepseek-v4-flash",
                            "status": "completed",
                            "output": [
                                {
                                    "type": "message",
                                    "id": "msg_1",
                                    "role": "assistant",
                                    "content": [{ "type": "output_text", "text": "Hello", "annotations": [] }]
                                }
                            ],
                            "usage": {
                                "input_tokens": 10,
                                "output_tokens": 3,
                                "total_tokens": 13,
                                "input_tokens_details": { "cached_tokens": 8 },
                                "output_tokens_details": { "reasoning_tokens": 0 }
                            }
                        }
                    }),
                )
                .unwrap(),
        );

        let halt = protocol.on_halt(&state);
        let finish = halt
            .iter()
            .find_map(|e| match e {
                LlmEvent::Finish { reason, usage } => Some((reason, usage)),
                _ => None,
            })
            .expect("finish event");
        assert_eq!(finish.0, &FinishReason::Stop);
        let usage = finish.1.as_ref().unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.cached_tokens, 8);

        let response = protocol.finalize(state).unwrap();
        assert_eq!(response.content, "Hello");
    }

    #[test]
    fn streaming_reasoning_and_tool_call_arguments_accumulate() {
        let protocol = OpenAiResponsesProtocol;
        let mut state = protocol.initial_state(&request());

        let mut events = Vec::new();
        events.extend(
            protocol
                .step(&mut state, &json!({"type": "response.reasoning_text.delta", "delta": "think", "output_index": 0}))
                .unwrap(),
        );
        events.extend(
            protocol
                .step(
                    &mut state,
                    &json!({
                        "type": "response.output_item.added",
                        "output_index": 1,
                        "item": {
                            "type": "function_call",
                            "id": "fc_1",
                            "call_id": "call_9",
                            "name": "web_search",
                            "arguments": ""
                        }
                    }),
                )
                .unwrap(),
        );
        events.extend(
            protocol
                .step(&mut state, &json!({"type": "response.function_call_arguments.delta", "delta": r#"{"q":"ru"#, "output_index": 1}))
                .unwrap(),
        );
        events.extend(
            protocol
                .step(&mut state, &json!({"type": "response.function_call_arguments.delta", "delta": r#"st"}"#, "output_index": 1}))
                .unwrap(),
        );
        events.extend(
            protocol
                .step(&mut state, &json!({"type": "response.function_call_arguments.done", "arguments": r#"{"q":"rust"}"#, "output_index": 1}))
                .unwrap(),
        );

        assert!(events
            .iter()
            .any(|e| matches!(e, LlmEvent::ReasoningStart { .. })));
        assert_eq!(state.accumulated_reasoning, "think");
        assert_eq!(state.tool_accumulators[1].call_id, "call_9");
        assert_eq!(state.tool_accumulators[1].name, "web_search");
        assert_eq!(state.tool_accumulators[1].arguments, r#"{"q":"rust"}"#);

        // Terminal event carries the full object; already-accumulated calls win.
        events.extend(
            protocol
                .step(
                    &mut state,
                    &json!({
                        "type": "response.completed",
                        "response": {
                            "id": "r1",
                            "status": "completed",
                            "output": [
                                { "type": "function_call", "id": "fc_1", "call_id": "call_9", "name": "web_search", "arguments": r#"{"q":"rust"}"# }
                            ],
                            "usage": { "input_tokens": 5, "output_tokens": 5, "total_tokens": 10 }
                        }
                    }),
                )
                .unwrap(),
        );

        let halt = protocol.on_halt(&state);
        assert!(halt
            .iter()
            .any(|e| matches!(e, LlmEvent::Finish { reason: FinishReason::ToolCalls, .. })));

        let response = protocol.finalize(state).unwrap();
        // No output text arrived; reasoning text becomes the content
        // (same fallback as openai_chat) while the tool call is preserved.
        assert_eq!(response.content, "think");
        assert_eq!(response.reasoning_content.as_deref(), Some("think"));
        let calls = response.tool_calls.unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "web_search");
        assert_eq!(calls[0].args["q"], "rust");
    }

    #[test]
    fn failed_response_yields_provider_error() {
        let protocol = OpenAiResponsesProtocol;
        let mut state = protocol.initial_state(&request());

        protocol
            .step(
                &mut state,
                &json!({
                    "type": "response.failed",
                    "response": {
                        "id": "r1",
                        "status": "failed",
                        "error": { "message": "rate limited", "code": "rate_limit_exceeded" },
                        "output": [],
                        "usage": { "input_tokens": 1, "output_tokens": 0, "total_tokens": 1 }
                    }
                }),
            )
            .unwrap();

        let halt = protocol.on_halt(&state);
        assert!(halt.iter().any(|e| matches!(e, LlmEvent::ProviderError { message, .. } if message == "rate limited")));
        assert!(protocol.finalize(state).is_err());
    }

    #[test]
    fn incomplete_response_is_length_finish() {
        let protocol = OpenAiResponsesProtocol;
        let mut state = protocol.initial_state(&request());
        protocol
            .step(&mut state, &json!({"type": "response.output_text.delta", "delta": "partial", "output_index": 0}))
            .unwrap();
        protocol
            .step(
                &mut state,
                &json!({
                    "type": "response.incomplete",
                    "response": { "id": "r1", "status": "incomplete", "output": [], "usage": { "input_tokens": 1, "output_tokens": 2, "total_tokens": 3 } }
                }),
            )
            .unwrap();

        let halt = protocol.on_halt(&state);
        assert!(halt
            .iter()
            .any(|e| matches!(e, LlmEvent::Finish { reason: FinishReason::Length, .. })));
    }

    #[test]
    fn empty_stream_is_provider_error() {
        let protocol = OpenAiResponsesProtocol;
        let mut state = protocol.initial_state(&request());
        protocol
            .step(
                &mut state,
                &json!({ "type": "response.completed", "response": { "id": "r1", "status": "completed", "output": [], "usage": { "input_tokens": 1, "output_tokens": 0, "total_tokens": 1 } } }),
            )
            .unwrap();
        let halt = protocol.on_halt(&state);
        assert!(halt
            .iter()
            .any(|e| matches!(e, LlmEvent::ProviderError { .. })));
    }

    #[test]
    fn deepseek_reasoning_content_blocks_are_absorbed() {
        let protocol = OpenAiResponsesProtocol;
        let mut state = protocol.initial_state(&request());

        let body = json!({
            "id": "resp_1",
            "status": "completed",
            "output": [
                {
                    "type": "reasoning",
                    "id": "rs_1",
                    "status": "completed",
                    "content": [{ "type": "reasoning_text", "text": "Let me think." }],
                    "summary": []
                },
                {
                    "type": "message",
                    "id": "msg_1",
                    "role": "assistant",
                    "content": [{ "type": "output_text", "text": "Answer.", "annotations": [] }]
                }
            ],
            "usage": { "input_tokens": 4, "output_tokens": 4, "total_tokens": 8 }
        });

        let events = protocol.step(&mut state, &body).unwrap();
        assert!(events
            .iter()
            .any(|e| matches!(e, LlmEvent::ReasoningDelta { text, .. } if text == "Let me think.")));

        let response = protocol.finalize(state).unwrap();
        assert_eq!(response.content, "Answer.");
        assert_eq!(response.reasoning_content.as_deref(), Some("Let me think."));
    }

    #[test]
    fn streaming_tool_call_without_item_added_backfills_identity() {
        // Provider sends `function_call_arguments.delta` but no
        // `output_item.added`; the terminal event must backfill call_id/name
        // into the delta-accumulated slot instead of duplicating the call.
        let protocol = OpenAiResponsesProtocol;
        let mut state = protocol.initial_state(&request());

        protocol
            .step(&mut state, &json!({"type": "response.function_call_arguments.delta", "delta": r#"{"q":"rust"}"#, "output_index": 0}))
            .unwrap();
        protocol
            .step(
                &mut state,
                &json!({
                    "type": "response.completed",
                    "response": {
                        "id": "r1",
                        "status": "completed",
                        "output": [
                            { "type": "function_call", "id": "fc_1", "call_id": "call_9", "name": "web_search", "arguments": r#"{"q":"rust"}"# }
                        ],
                        "usage": { "input_tokens": 5, "output_tokens": 5, "total_tokens": 10 }
                    }
                }),
            )
            .unwrap();

        let halt = protocol.on_halt(&state);
        assert!(halt
            .iter()
            .any(|e| matches!(e, LlmEvent::Finish { reason: FinishReason::ToolCalls, .. })));

        let response = protocol.finalize(state).unwrap();
        let calls = response.tool_calls.expect("expected exactly one tool call");
        assert_eq!(calls.len(), 1, "duplicate tool call: {calls:?}");
        assert_eq!(calls[0].tool, "web_search");
        assert_eq!(calls[0].args["q"], "rust");
    }

    #[test]
    fn protocol_id_and_state_defaults() {
        let protocol = OpenAiResponsesProtocol;
        assert_eq!(protocol.protocol_id(), "openai_responses");
        let state = protocol.initial_state(&request());
        assert_eq!(state.provider, "deepseek");
        assert_eq!(state.model, "deepseek-v4-flash");
    }

    #[test]
    fn decode_frame_rejects_garbage_and_accepts_done() {
        let protocol = OpenAiResponsesProtocol;
        assert!(protocol.decode_frame("not json").is_err());
        assert!(protocol.decode_frame("[DONE]").is_ok());
        let value = protocol.decode_frame(r#"{"type":"response.created","response":{"id":"r"}}"#).unwrap();
        assert_eq!(value["type"], "response.created");
    }
}
