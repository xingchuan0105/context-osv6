use crate::ModelProviderConfig;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

fn build_chat_completion_request_body(
    config: &ModelProviderConfig,
    messages: &[ChatMessage],
    temperature: Option<f32>,
    stream: bool,
) -> serde_json::Value {
    let mut request_body = serde_json::json!({
        "model": config.model,
        "messages": messages
            .iter()
            .map(|m| serde_json::json!({
                "role": m.role,
                "content": m.content
            }))
            .collect::<Vec<_>>(),
    });

    if let Some(temp) = temperature {
        request_body["temperature"] = serde_json::json!(temp);
    }
    if let Some(enable_thinking) = config.enable_thinking {
        if config.base_url.to_ascii_lowercase().contains("deepseek") {
            let mut thinking = serde_json::json!({
                "type": if enable_thinking { "enabled" } else { "disabled" },
            });
            if enable_thinking {
                thinking["reasoning_effort"] = serde_json::json!("max");
            }
            request_body["thinking"] = thinking;
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

    request_body
}

#[derive(Debug, Deserialize)]
struct StreamChoiceDelta {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    #[serde(default)]
    delta: Option<StreamChoiceDelta>,
}

#[derive(Debug, Deserialize)]
struct StreamUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct StreamChunk {
    #[serde(default)]
    choices: Vec<StreamChoice>,
    #[serde(default)]
    usage: Option<StreamUsage>,
    #[serde(default)]
    model: Option<String>,
}

#[derive(Debug)]
struct ChatCompletionStreamParser {
    buffer: Vec<u8>,
    data_lines: Vec<String>,
    accumulated_content: String,
    usage: Option<LlmUsage>,
    model: String,
    provider: String,
}

impl ChatCompletionStreamParser {
    fn new(provider: String, configured_model: String) -> Self {
        Self {
            buffer: Vec::new(),
            data_lines: Vec::new(),
            accumulated_content: String::new(),
            usage: None,
            model: configured_model,
            provider,
        }
    }

    fn feed_chunk(&mut self, chunk: &[u8], on_delta: &mut impl FnMut(&str)) -> anyhow::Result<()> {
        self.buffer.extend_from_slice(chunk);

        while let Some(line) = self.take_line()? {
            if line.is_empty() {
                self.flush_event(on_delta)?;
                continue;
            }

            if line.starts_with(':') {
                continue;
            }

            if let Some(value) = line.strip_prefix("data:") {
                self.data_lines.push(value.trim_start().to_string());
            }
        }

        Ok(())
    }

    fn finish(mut self, on_delta: &mut impl FnMut(&str)) -> anyhow::Result<LlmResponse> {
        if !self.buffer.is_empty() {
            let line = String::from_utf8(std::mem::take(&mut self.buffer))
                .context("Failed to decode trailing chat completion stream line")?;
            let normalized = line.trim_end_matches('\r');
            if let Some(value) = normalized.strip_prefix("data:") {
                self.data_lines.push(value.trim_start().to_string());
            }
        }

        self.flush_event(on_delta)?;

        if self.accumulated_content.is_empty() {
            anyhow::bail!("Chat completion stream finished without content");
        }

        Ok(LlmResponse {
            content: self.accumulated_content,
            usage: self.usage.unwrap_or_else(|| LlmUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
                provider: self.provider,
                model: self.model.clone(),
                cached_tokens: 0,
            }),
            model: self.model,
        })
    }

    fn take_line(&mut self) -> anyhow::Result<Option<String>> {
        let Some(index) = self.buffer.iter().position(|byte| *byte == b'\n') else {
            return Ok(None);
        };

        let mut raw_line = self.buffer.drain(..=index).collect::<Vec<_>>();
        raw_line.pop();
        if raw_line.last() == Some(&b'\r') {
            raw_line.pop();
        }

        let line =
            String::from_utf8(raw_line).context("Failed to decode chat completion stream line")?;
        Ok(Some(line))
    }

    fn flush_event(&mut self, on_delta: &mut impl FnMut(&str)) -> anyhow::Result<()> {
        if self.data_lines.is_empty() {
            return Ok(());
        }

        let payload = self.data_lines.join("\n");
        self.data_lines.clear();

        if payload.trim() == "[DONE]" {
            return Ok(());
        }

        let chunk: StreamChunk = serde_json::from_str(&payload).with_context(|| {
            format!("Failed to parse chat completion stream payload: {payload}")
        })?;

        if let Some(model) = chunk.model {
            self.model = model;
        }

        if let Some(usage) = chunk.usage {
            self.usage = Some(LlmUsage {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                total_tokens: usage.total_tokens,
                provider: self.provider.clone(),
                model: self.model.clone(),
                cached_tokens: 0,
            });
        }

        for choice in chunk.choices {
            let Some(delta) = choice.delta else {
                continue;
            };
            let Some(content) = delta.content else {
                continue;
            };

            if content.is_empty() {
                continue;
            }

            self.accumulated_content.push_str(&content);
            on_delta(&content);
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct LlmClient {
    pub config: ModelProviderConfig,
    client: reqwest::Client,
    rate_limiter: Option<crate::SharedRateLimiter>,
}

impl LlmClient {
    pub fn new(config: ModelProviderConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(config.timeout_ms))
            .build()
            .expect("reqwest client should build");
        let rate_limiter = if config.is_configured() {
            let rpm = config.effective_rpm_limit();
            let tpm = config.effective_tpm_limit();
            Some(std::sync::Arc::new(crate::RateLimiter::new(rpm, tpm)))
        } else {
            None
        };
        Self {
            config,
            client,
            rate_limiter,
        }
    }

    fn estimate_input_tokens(&self, messages: &[ChatMessage]) -> usize {
        crate::count_chat_messages(messages)
    }

    fn check_rate_limit(&self, estimated_tokens: usize) -> anyhow::Result<usize> {
        if let Some(limiter) = &self.rate_limiter {
            match limiter.check_request(estimated_tokens) {
                Ok(deducted) => Ok(deducted),
                Err(crate::RateLimitError::RpmExceeded) => {
                    anyhow::bail!("LLM rate limit exceeded: too many requests per minute")
                }
                Err(crate::RateLimitError::TpmExceeded) => {
                    anyhow::bail!("LLM rate limit exceeded: too many tokens per minute")
                }
            }
        } else {
            Ok(estimated_tokens)
        }
    }

    fn record_usage(&self, pre_deducted: usize, actual_tokens: usize) {
        if let Some(limiter) = &self.rate_limiter {
            limiter.record_actual_usage(pre_deducted, actual_tokens);
        }
    }

    /// Send a chat completion request
    pub async fn complete(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
    ) -> anyhow::Result<LlmResponse> {
        let started_at = std::time::Instant::now();
        let provider = self.config.provider_name();
        let configured_model = self.config.model.clone();
        if !self.config.is_configured() {
            telemetry::prometheus::observe_llm_call(
                "generic",
                &provider,
                &configured_model,
                "failure",
                started_at.elapsed().as_secs_f64() * 1000.0,
            );
            anyhow::bail!("LLM not configured");
        }

        let request_body =
            build_chat_completion_request_body(&self.config, messages, temperature, false);

        let estimated_tokens = self.estimate_input_tokens(messages);
        let pre_deducted = self.check_rate_limit(estimated_tokens)?;

        let response = self
            .client
            .post(format!("{}/chat/completions", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                telemetry::prometheus::record_dependency_failure(&provider);
                telemetry::prometheus::observe_llm_call(
                    "generic",
                    &provider,
                    &configured_model,
                    "failure",
                    started_at.elapsed().as_secs_f64() * 1000.0,
                );
                return Err(error).context("Failed to send chat completion request");
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            telemetry::prometheus::record_dependency_failure(&provider);
            telemetry::prometheus::observe_llm_call(
                "generic",
                &provider,
                &configured_model,
                "failure",
                started_at.elapsed().as_secs_f64() * 1000.0,
            );
            anyhow::bail!("Chat completion API error {}: {}", status, body);
        }

        #[derive(serde::Deserialize)]
        struct Choice {
            message: ResponseMessage,
        }

        #[derive(serde::Deserialize)]
        struct ResponseMessage {
            content: String,
        }

        #[derive(serde::Deserialize)]
        struct Usage {
            prompt_tokens: u32,
            completion_tokens: u32,
            total_tokens: u32,
        }

        #[derive(serde::Deserialize)]
        struct CompletionResponse {
            choices: Vec<Choice>,
            usage: Usage,
            model: String,
        }

        let resp = response.json().await;
        let resp: CompletionResponse = match resp {
            Ok(resp) => resp,
            Err(error) => {
                telemetry::prometheus::record_dependency_failure(&provider);
                telemetry::prometheus::observe_llm_call(
                    "generic",
                    &provider,
                    &configured_model,
                    "failure",
                    started_at.elapsed().as_secs_f64() * 1000.0,
                );
                return Err(error).context("Failed to parse chat completion response");
            }
        };

        let content = resp
            .choices
            .first()
            .context("No choices in response")?
            .message
            .content
            .clone();
        telemetry::prometheus::observe_llm_call(
            "generic",
            &provider,
            &resp.model,
            "success",
            started_at.elapsed().as_secs_f64() * 1000.0,
        );
        telemetry::prometheus::observe_llm_usage(
            "generic",
            &provider,
            &resp.model,
            resp.usage.prompt_tokens as u64,
            resp.usage.completion_tokens as u64,
        );

        self.record_usage(pre_deducted, resp.usage.total_tokens as usize);

        Ok(LlmResponse {
            content,
            usage: LlmUsage {
                prompt_tokens: resp.usage.prompt_tokens,
                completion_tokens: resp.usage.completion_tokens,
                total_tokens: resp.usage.total_tokens,
                provider: self.config.provider_name(),
                model: resp.model.clone(),
                cached_tokens: 0,
            },
            model: resp.model,
        })
    }

    pub async fn complete_stream(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
        token: CancellationToken,
        mut on_delta: impl FnMut(&str),
    ) -> anyhow::Result<LlmResponse> {
        let started_at = std::time::Instant::now();
        let provider = self.config.provider_name();
        let configured_model = self.config.model.clone();
        if !self.config.is_configured() {
            telemetry::prometheus::observe_llm_call(
                "generic",
                &provider,
                &configured_model,
                "failure",
                started_at.elapsed().as_secs_f64() * 1000.0,
            );
            anyhow::bail!("LLM not configured");
        }

        let request_body =
            build_chat_completion_request_body(&self.config, messages, temperature, true);

        let estimated_tokens = self.estimate_input_tokens(messages);
        let pre_deducted = self.check_rate_limit(estimated_tokens)?;

        let request = self
            .client
            .post(format!("{}/chat/completions", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(&request_body);

        let response = tokio::select! {
            res = request.send() => res,
            _ = token.cancelled() => anyhow::bail!("LLM request cancelled"),
        };

        let mut response = match response {
            Ok(response) => response,
            Err(error) => {
                telemetry::prometheus::record_dependency_failure(&provider);
                telemetry::prometheus::observe_llm_call(
                    "generic",
                    &provider,
                    &configured_model,
                    "failure",
                    started_at.elapsed().as_secs_f64() * 1000.0,
                );
                return Err(anyhow::Error::new(error)).context("Failed to send chat completion stream request");
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            telemetry::prometheus::record_dependency_failure(&provider);
            telemetry::prometheus::observe_llm_call(
                "generic",
                &provider,
                &configured_model,
                "failure",
                started_at.elapsed().as_secs_f64() * 1000.0,
            );
            anyhow::bail!("Chat completion stream API error {}: {}", status, body);
        }

        let mut parser =
            ChatCompletionStreamParser::new(provider.clone(), configured_model.clone());

        loop {
            let next_chunk = tokio::select! {
                chunk = response.chunk() => chunk.context("Failed to read chat completion stream chunk")?,
                _ = token.cancelled() => anyhow::bail!("LLM request cancelled"),
            };
            let Some(chunk) = next_chunk else {
                break;
            };

            parser.feed_chunk(&chunk, &mut on_delta)?;
        }

        let parsed = parser.finish(&mut on_delta)?;

        telemetry::prometheus::observe_llm_call(
            "generic",
            &provider,
            &parsed.model,
            "success",
            started_at.elapsed().as_secs_f64() * 1000.0,
        );
        telemetry::prometheus::observe_llm_usage(
            "generic",
            &provider,
            &parsed.model,
            parsed.usage.prompt_tokens as u64,
            parsed.usage.completion_tokens as u64,
        );

        self.record_usage(pre_deducted, parsed.usage.total_tokens as usize);

        Ok(parsed)
    }
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: String,
    pub usage: LlmUsage,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    /// Tokens served from prompt cache (when provider supports it).
    #[serde(default)]
    pub cached_tokens: u32,
}

impl LlmUsage {
    pub fn zeroed() -> Self {
        Self {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            provider: String::new(),
            model: String::new(),
            cached_tokens: 0,
        }
    }

    pub fn accumulate(&mut self, other: &LlmUsage) {
        self.prompt_tokens += other.prompt_tokens;
        self.completion_tokens += other.completion_tokens;
        self.total_tokens += other.total_tokens;
        self.cached_tokens += other.cached_tokens;
        if self.provider.is_empty() && !other.provider.is_empty() {
            self.provider = other.provider.clone();
        }
        if self.model.is_empty() && !other.model.is_empty() {
            self.model = other.model.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ChatCompletionStreamParser, ChatMessage, LlmUsage, build_chat_completion_request_body,
    };
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
    fn llm_usage_accumulate_preserves_provider_and_model() {
        let mut total = LlmUsage::zeroed();
        total.accumulate(&LlmUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
            provider: "dmxapi".to_string(),
            model: "gemini-test".to_string(),
            cached_tokens: 5,
        });

        assert_eq!(total.total_tokens, 30);
        assert_eq!(total.provider, "dmxapi");
        assert_eq!(total.model, "gemini-test");
    }

    #[test]
    fn deepseek_request_maps_enable_thinking_to_thinking_object() {
        let config = test_config("https://api.deepseek.com", Some(false));
        let body = build_chat_completion_request_body(
            &config,
            &[ChatMessage::user("hello")],
            Some(0.3),
            false,
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
        );

        assert_eq!(body["thinking"]["type"], "enabled");
        assert_eq!(body["thinking"]["reasoning_effort"], "max");
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
        );

        assert_eq!(body["enable_thinking"], false);
        assert!(body.get("thinking").is_none());
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
            )
            .unwrap();
        parser
            .feed_chunk(
                br#"data: {"choices":[{"delta":{"content":"lo"}}]}

data: {"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":3,"total_tokens":15}}

data: [DONE]

"#,
                &mut |delta| observed.push_str(delta),
            )
            .unwrap();

        let response = parser
            .finish(&mut |delta| observed.push_str(delta))
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
            )
            .unwrap();
        parser
            .feed_chunk(
                br#"B"}}]}

data: [DONE]

"#,
                &mut |delta| observed.push_str(delta),
            )
            .unwrap();

        let response = parser
            .finish(&mut |delta| observed.push_str(delta))
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
            )
            .unwrap();

        let error = parser.finish(&mut |_delta| {}).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("Chat completion stream finished without content")
        );
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
        );
        assert!(body.get("prompt_cache").is_none());
    }
}
