mod rate_limit;
mod request;
mod stream_parser;
mod types;

use crate::ModelProviderConfig;
use anyhow::Context;
use rate_limit::ClientRateLimit;
use request::build_chat_completion_request_body;
use stream_parser::{ApiUsageRaw, ChatCompletionStreamParser};
use tokio_util::sync::CancellationToken;

pub use types::{ChatMessage, ContentPart, ImageUrlDetail, LlmResponse, LlmUsage};

struct CompletionCall {
    started_at: std::time::Instant,
    provider: String,
    configured_model: String,
    pre_deducted: usize,
}

#[derive(Debug, Clone)]
pub struct LlmClient {
    pub config: ModelProviderConfig,
    client: reqwest::Client,
    rate_limit: ClientRateLimit,
}

impl LlmClient {
    pub fn new(config: ModelProviderConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(config.timeout_ms))
            .build()
            .expect("reqwest client should build");
        let rate_limit = ClientRateLimit::from_config(&config);
        Self {
            config,
            client,
            rate_limit,
        }
    }

    fn prepare_completion(&self, messages: &[ChatMessage]) -> anyhow::Result<CompletionCall> {
        let started_at = std::time::Instant::now();
        let provider = self.config.provider_name();
        let configured_model = self.config.model.clone();
        if !self.config.is_configured() {
            Self::record_completion_failure(&provider, &configured_model, started_at);
            anyhow::bail!("LLM not configured");
        }

        let estimated_tokens = self.rate_limit.estimate_input_tokens(messages);
        let pre_deducted = self.rate_limit.check_rate_limit(estimated_tokens)?;

        Ok(CompletionCall {
            started_at,
            provider,
            configured_model,
            pre_deducted,
        })
    }

    fn build_completion_request_body(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
        stream: bool,
        tools: Option<&[contracts::ToolSpec]>,
        json_mode: bool,
        max_tokens: Option<u32>,
    ) -> serde_json::Value {
        let mut request_body = build_chat_completion_request_body(
            &self.config,
            messages,
            temperature,
            stream,
            json_mode,
            max_tokens,
        );

        if let Some(tools) = tools {
            if !tools.is_empty() {
                let openai_tools = tools
                    .iter()
                    .map(|spec| {
                        serde_json::json!({
                            "type": "function",
                            "function": {
                                "name": spec.name,
                                "description": spec.description,
                                "parameters": spec.input_schema,
                            }
                        })
                    })
                    .collect::<Vec<_>>();
                request_body["tools"] = serde_json::json!(openai_tools);
            }
        }

        request_body
    }

    fn record_call_failure(call: &CompletionCall) {
        Self::record_dependency_failure(&call.provider);
        Self::record_completion_failure(&call.provider, &call.configured_model, call.started_at);
    }

    fn record_completion_failure(
        provider: &str,
        configured_model: &str,
        started_at: std::time::Instant,
    ) {
        telemetry::prometheus::observe_llm_call(
            "generic",
            provider,
            configured_model,
            "failure",
            started_at.elapsed().as_secs_f64() * 1000.0,
        );
    }

    fn record_dependency_failure(provider: &str) {
        telemetry::prometheus::record_dependency_failure(provider);
    }

    async fn post_chat_completions(
        &self,
        call: &CompletionCall,
        request_body: &serde_json::Value,
        stream: bool,
        cancel_token: Option<&CancellationToken>,
    ) -> anyhow::Result<reqwest::Response> {
        let mut request = self
            .client
            .post(format!("{}/chat/completions", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json");
        if stream {
            request = request.header("Accept", "text/event-stream");
        }
        let request = request.json(request_body);

        let response = if let Some(token) = cancel_token {
            tokio::select! {
                res = request.send() => res,
                _ = token.cancelled() => anyhow::bail!("LLM request cancelled"),
            }
        } else {
            request.send().await
        };

        match response {
            Ok(response) => {
                if response.status().is_success() {
                    Ok(response)
                } else {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    Self::record_call_failure(call);
                    if stream {
                        anyhow::bail!("Chat completion stream API error {}: {}", status, body);
                    } else {
                        anyhow::bail!("Chat completion API error {}: {}", status, body);
                    }
                }
            }
            Err(error) => {
                Self::record_call_failure(call);
                let context = if stream {
                    "Failed to send chat completion stream request"
                } else {
                    "Failed to send chat completion request"
                };
                Err(anyhow::Error::new(error)).context(context)
            }
        }
    }

    fn record_completion_success(
        &self,
        call: &CompletionCall,
        model: &str,
        usage: &ApiUsageRaw,
        cached_tokens_for_metrics: u64,
    ) {
        telemetry::prometheus::observe_llm_call(
            "generic",
            &call.provider,
            model,
            "success",
            call.started_at.elapsed().as_secs_f64() * 1000.0,
        );
        telemetry::prometheus::observe_llm_usage(
            "generic",
            &call.provider,
            model,
            usage.prompt_tokens() as u64,
            usage.completion_tokens() as u64,
            cached_tokens_for_metrics,
        );
        self.rate_limit
            .record_usage(call.pre_deducted, usage.total_tokens() as usize);
    }

    async fn complete_non_stream(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
        tools: Option<&[contracts::ToolSpec]>,
        json_mode: bool,
        max_tokens: Option<u32>,
    ) -> anyhow::Result<LlmResponse> {
        let call = self.prepare_completion(messages)?;
        let request_body = self.build_completion_request_body(
            messages,
            temperature,
            false,
            tools,
            json_mode,
            max_tokens,
        );

        let response = self
            .post_chat_completions(&call, &request_body, false, None)
            .await?;

        #[derive(serde::Deserialize)]
        struct Choice {
            message: ResponseMessage,
        }

        #[derive(serde::Deserialize)]
        struct ResponseMessage {
            #[serde(default)]
            content: Option<String>,
            #[serde(default)]
            reasoning_content: Option<String>,
            #[serde(default)]
            tool_calls: Option<Vec<OpenAiToolCall>>,
        }

        #[allow(dead_code)]
        #[derive(serde::Deserialize)]
        struct OpenAiToolCall {
            id: String,
            #[serde(rename = "type")]
            call_type: String,
            #[serde(default)]
            function: Option<OpenAiFunctionCall>,
        }

        #[derive(serde::Deserialize)]
        struct OpenAiFunctionCall {
            name: String,
            arguments: String,
        }

        #[derive(serde::Deserialize)]
        struct CompletionResponse {
            choices: Vec<Choice>,
            usage: ApiUsageRaw,
            model: String,
        }

        let resp = response.json().await;
        let resp: CompletionResponse = match resp {
            Ok(resp) => resp,
            Err(error) => {
                Self::record_call_failure(&call);
                return Err(error).context("Failed to parse chat completion response");
            }
        };

        let choice = resp.choices.first().context("No choices in response")?;
        let content = choice.message.content.clone().unwrap_or_default();
        let reasoning_content = choice.message.reasoning_content.clone();

        let tool_calls = if let Some(ref calls) = choice.message.tool_calls {
            let mut mapped_calls = Vec::new();
            for tool_call in calls {
                if let Some(ref func) = tool_call.function {
                    let args = serde_json::from_str(&func.arguments)
                        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                    mapped_calls.push(contracts::ToolCall {
                        tool: func.name.clone(),
                        version: "1.0".to_string(),
                        args,
                    });
                }
            }
            if mapped_calls.is_empty() {
                None
            } else {
                Some(mapped_calls)
            }
        } else {
            None
        };

        self.record_completion_success(
            &call,
            &resp.model,
            &resp.usage,
            resp.usage.cached_token_count() as u64,
        );

        let llm_usage = resp
            .usage
            .to_llm_usage(self.config.provider_name(), resp.model.clone());

        Ok(LlmResponse {
            content,
            reasoning_content,
            usage: llm_usage,
            model: resp.model,
            tool_calls,
        })
    }

    /// Send a chat completion request with tool specifications
    pub async fn complete_with_tools(
        &self,
        messages: &[ChatMessage],
        tools: &[contracts::ToolSpec],
        temperature: Option<f32>,
    ) -> anyhow::Result<LlmResponse> {
        self.complete_non_stream(messages, temperature, Some(tools), false, None)
            .await
    }

    /// Send a chat completion request
    pub async fn complete(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
    ) -> anyhow::Result<LlmResponse> {
        self.complete_non_stream(messages, temperature, None, false, None)
            .await
    }

    /// Send a chat completion request with an explicit output token cap.
    pub async fn complete_with_max_tokens(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
        max_tokens: u32,
    ) -> anyhow::Result<LlmResponse> {
        self.complete_non_stream(messages, temperature, None, false, Some(max_tokens))
            .await
    }

    /// Send a chat completion request with DeepSeek JSON Output enabled
    /// (`response_format: json_object`). Use this for synthesis turns that must
    /// emit a structured `internal_answer_v1` / `internal_search_answer_v1`
    /// JSON object. The prompt must already contain the word "json" and a
    /// format example (see `synthesis_contract_block`). On non-DeepSeek
    /// providers this silently falls back to a normal completion.
    pub async fn complete_json_mode(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
    ) -> anyhow::Result<LlmResponse> {
        self.complete_non_stream(messages, temperature, None, true, None)
            .await
    }

    pub async fn complete_stream(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
        token: CancellationToken,
        mut on_content_delta: impl FnMut(&str),
        mut on_reasoning_delta: impl FnMut(&str),
    ) -> anyhow::Result<LlmResponse> {
        let call = self.prepare_completion(messages)?;
        let request_body = self.build_completion_request_body(
            messages,
            temperature,
            true,
            None,
            false,
            None,
        );

        let mut response = self
            .post_chat_completions(&call, &request_body, true, Some(&token))
            .await?;

        let mut parser =
            ChatCompletionStreamParser::new(call.provider.clone(), call.configured_model.clone());

        loop {
            let next_chunk = tokio::select! {
                chunk = response.chunk() => chunk.context("Failed to read chat completion stream chunk")?,
                _ = token.cancelled() => anyhow::bail!("LLM request cancelled"),
            };
            let Some(chunk) = next_chunk else {
                break;
            };

            parser.feed_chunk(&chunk, &mut on_content_delta, &mut on_reasoning_delta)?;
        }

        let parsed = parser.finish(&mut on_content_delta, &mut on_reasoning_delta)?;

        self.record_completion_success(
            &call,
            &parsed.model,
            &ApiUsageRaw::from_token_counts(
                parsed.usage.prompt_tokens,
                parsed.usage.completion_tokens,
                parsed.usage.total_tokens,
                parsed.usage.cached_tokens,
            ),
            parsed.usage.cached_tokens as u64,
        );

        Ok(parsed)
    }
}
