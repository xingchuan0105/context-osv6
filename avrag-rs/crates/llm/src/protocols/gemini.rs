use super::Protocol;
use crate::schema::{
    FinishReason, LlmError, LlmEvent, LlmRequest, LlmResponse, LlmUsage, Usage,
};
use serde::Deserialize;

const TEXT_BLOCK_ID: &str = "text-0";

#[derive(Debug, Clone, Copy, Default)]
pub struct GeminiProtocol;

#[derive(Debug, Default)]
pub struct GeminiState {
    accumulated_content: String,
    usage: Option<LlmUsage>,
    model: String,
    provider: String,
    #[allow(dead_code)]
    configured_model: String,
    text_started: bool,
}

pub fn build_gemini_body(req: &LlmRequest) -> Result<serde_json::Value, LlmError> {
    let mut system_instruction = None;
    let mut contents = Vec::new();

    for message in &req.messages {
        if message.role == "system" {
            system_instruction = Some(serde_json::json!({
                "parts": [{"text": message.content}],
            }));
            continue;
        }

        let role = if message.role == "assistant" {
            "model"
        } else {
            "user"
        };

        let mut parts = Vec::new();
        if let Some(ref multimodal) = message.multimodal_content {
            for part in multimodal {
                match part {
                    crate::schema::ContentPart::Text { text } => {
                        parts.push(serde_json::json!({"text": text}));
                    }
                    crate::schema::ContentPart::ImageUrl { image_url } => {
                        parts.push(serde_json::json!({
                            "file_data": {
                                "mime_type": "image/jpeg",
                                "file_uri": image_url.url,
                            }
                        }));
                    }
                }
            }
        } else if !message.content.is_empty() {
            parts.push(serde_json::json!({"text": message.content}));
        }

        if parts.is_empty() {
            parts.push(serde_json::json!({"text": ""}));
        }

        contents.push(serde_json::json!({
            "role": role,
            "parts": parts,
        }));
    }

    let mut generation_config = serde_json::Map::new();
    if let Some(temp) = req.options.temperature {
        generation_config.insert("temperature".into(), serde_json::json!(temp));
    }
    if let Some(max_tokens) = req.options.max_tokens {
        generation_config.insert("maxOutputTokens".into(), serde_json::json!(max_tokens));
    }
    if req.options.json_mode {
        generation_config.insert("responseMimeType".into(), serde_json::json!("application/json"));
    }

    let mut body = serde_json::json!({ "contents": contents });
    if !generation_config.is_empty() {
        body["generationConfig"] = serde_json::Value::Object(generation_config);
    }
    if let Some(system) = system_instruction {
        body["systemInstruction"] = system;
    }

    Ok(body)
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct GeminiUsageMetadata {
    #[serde(default)]
    prompt_token_count: u32,
    #[serde(default)]
    candidates_token_count: u32,
    #[serde(default)]
    total_token_count: u32,
}

fn usage_from_gemini(usage: &GeminiUsageMetadata, provider: &str, model: &str) -> LlmUsage {
    let total = if usage.total_token_count > 0 {
        usage.total_token_count
    } else {
        usage.prompt_token_count + usage.candidates_token_count
    };
    LlmUsage {
        prompt_tokens: usage.prompt_token_count,
        completion_tokens: usage.candidates_token_count,
        total_tokens: total,
        provider: provider.to_string(),
        model: model.to_string(),
        cached_tokens: 0,
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

fn extract_text_from_response(value: &serde_json::Value) -> String {
    let mut content = String::new();
    if let Some(candidates) = value.get("candidates").and_then(|v| v.as_array()) {
        for candidate in candidates {
            if let Some(parts) = candidate
                .get("content")
                .and_then(|c| c.get("parts"))
                .and_then(|p| p.as_array())
            {
                for part in parts {
                    if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                        content.push_str(text);
                    }
                }
            }
        }
    }
    content
}

impl Protocol for GeminiProtocol {
    type Body = serde_json::Value;
    type State = GeminiState;

    fn protocol_id(&self) -> &'static str {
        "gemini"
    }

    fn build_body(&self, req: &LlmRequest) -> Result<Self::Body, LlmError> {
        build_gemini_body(req)
    }

    fn initial_state(&self, req: &LlmRequest) -> Self::State {
        GeminiState {
            provider: req.config.provider_name(),
            configured_model: req.config.model.clone(),
            model: req.config.model.clone(),
            ..Default::default()
        }
    }

    fn endpoint_path(&self, req: &LlmRequest) -> Option<String> {
        let action = if req.options.stream {
            "streamGenerateContent"
        } else {
            "generateContent"
        };
        Some(format!("/models/{}:{action}", req.config.model))
    }

    fn endpoint_query(&self, req: &LlmRequest) -> Vec<(String, String)> {
        if req.options.stream {
            vec![("alt".to_string(), "sse".to_string())]
        } else {
            Vec::new()
        }
    }

    fn decode_frame(&self, frame: &str) -> Result<serde_json::Value, LlmError> {
        if frame.trim().is_empty() || frame.trim() == "[DONE]" {
            return Ok(serde_json::Value::Null);
        }
        serde_json::from_str(frame).map_err(|error| {
            LlmError::parse(format!("Failed to parse Gemini stream payload: {frame}: {error}"))
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

        if let Some(model) = event.get("modelVersion").and_then(|v| v.as_str()) {
            state.model = model.to_string();
        }

        if let Some(usage) = event.get("usageMetadata") {
            let usage: GeminiUsageMetadata = serde_json::from_value(usage.clone())
                .map_err(|e| LlmError::parse(format!("invalid Gemini usage: {e}")))?;
            state.usage = Some(usage_from_gemini(
                &usage,
                &state.provider,
                &state.model,
            ));
        }

        let text = extract_text_from_response(event);
        if text.is_empty() {
            return Ok(Vec::new());
        }

        let mut events = Vec::new();
        if !state.text_started {
            state.text_started = true;
            events.push(LlmEvent::TextStart {
                id: TEXT_BLOCK_ID.to_string(),
            });
        }
        state.accumulated_content.push_str(&text);
        events.push(LlmEvent::TextDelta {
            id: TEXT_BLOCK_ID.to_string(),
            text,
        });
        Ok(events)
    }

    fn on_halt(&self, state: &Self::State) -> Vec<LlmEvent> {
        let mut events = Vec::new();

        if state.text_started {
            events.push(LlmEvent::TextEnd {
                id: TEXT_BLOCK_ID.to_string(),
            });
        }

        if state.accumulated_content.is_empty() {
            events.push(LlmEvent::ProviderError {
                message: "Gemini stream finished without content".to_string(),
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
        if state.accumulated_content.is_empty() {
            return Err(LlmError::EmptyStream);
        }

        Ok(LlmResponse {
            content: state.accumulated_content,
            reasoning_content: None,
            usage: state.usage.unwrap_or_else(|| LlmUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
                provider: state.provider,
                model: state.model.clone(),
                cached_tokens: 0,
            }),
            model: state.model,
            tool_calls: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{build_gemini_body, GeminiProtocol};
    use crate::schema::{ChatMessage, GenerationOptions, LlmRequest};
    use crate::ModelProviderConfig;
    use crate::protocols::Protocol;

    fn test_config() -> ModelProviderConfig {
        ModelProviderConfig {
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
            api_key: "test-key".to_string(),
            model: "gemini-2.0-flash".to_string(),
            timeout_ms: 1000,
            api_style: None,
            dimensions: None,
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        }
    }

    #[test]
    fn gemini_body_maps_roles_and_system_instruction() {
        let req = LlmRequest::new(
            vec![
                ChatMessage::system("Be concise."),
                ChatMessage::user("Hello"),
                ChatMessage::assistant("Hi"),
            ],
            test_config(),
        );
        let body = build_gemini_body(&req).unwrap();
        assert_eq!(body["contents"][0]["role"], "user");
        assert_eq!(body["contents"][1]["role"], "model");
        assert_eq!(body["systemInstruction"]["parts"][0]["text"], "Be concise.");
    }

    #[test]
    fn gemini_endpoint_path_includes_model_and_stream_action() {
        let protocol = GeminiProtocol;
        let req = LlmRequest::new(vec![ChatMessage::user("hi")], test_config())
            .with_options(GenerationOptions {
                stream: true,
                ..Default::default()
            });
        assert_eq!(
            protocol.endpoint_path(&req).unwrap(),
            "/models/gemini-2.0-flash:streamGenerateContent"
        );
        assert_eq!(protocol.endpoint_query(&req)[0].0, "alt");
    }

    #[test]
    fn gemini_non_stream_response_is_finalized() {
        let protocol = GeminiProtocol;
        let req = LlmRequest::new(vec![ChatMessage::user("hi")], test_config());
        let mut state = protocol.initial_state(&req);
        let response = serde_json::json!({
            "candidates": [{
                "content": {"parts": [{"text": "Hello"}]}
            }],
            "usageMetadata": {
                "promptTokenCount": 4,
                "candidatesTokenCount": 2,
                "totalTokenCount": 6
            }
        });
        protocol.step(&mut state, &response).unwrap();
        let finalized = protocol.finalize(state).unwrap();
        assert_eq!(finalized.content, "Hello");
        assert_eq!(finalized.usage.total_tokens, 6);
    }
}
