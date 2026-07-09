mod rate_limit;
mod stream_parser;
mod types;

use crate::protocols::Protocol;
use crate::route::build_route_from_config;
use crate::schema::{GenerationOptions, LlmEvent, LlmRequest, ToolDefinition};
use crate::usage_observer::{ChatUsageRecord, TenantContext, UsageObserver};
use crate::{AnyRoute, ModelProviderConfig};
use anyhow::Context;
use futures::StreamExt;
use rate_limit::ClientRateLimit;
use std::sync::Arc;
use stream_parser::ApiUsageRaw;
use tokio_util::sync::CancellationToken;

pub use types::{ChatMessage, ContentPart, ImageUrlDetail, LlmResponse, LlmUsage};

struct CompletionCall {
    started_at: std::time::Instant,
    provider: String,
    configured_model: String,
    pre_deducted: usize,
}

#[derive(Clone)]
pub struct LlmClient {
    pub config: ModelProviderConfig,
    route: AnyRoute,
    rate_limit: ClientRateLimit,
    feature: String,
    stage: String,
    observer: Option<(Arc<dyn UsageObserver>, TenantContext)>,
}

impl std::fmt::Debug for LlmClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmClient")
            .field("config", &self.config)
            .field("feature", &self.feature)
            .field("stage", &self.stage)
            .field("has_observer", &self.observer.is_some())
            .finish_non_exhaustive()
    }
}

impl LlmClient {
    pub fn new(config: ModelProviderConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(config.timeout_ms))
            .build()
            .expect("reqwest client should build");
        let rate_limit = ClientRateLimit::from_config(&config);
        let route = build_route_from_config(&config, client);
        Self {
            config,
            route,
            rate_limit,
            feature: "agent_loop".to_string(),
            stage: String::new(),
            observer: None,
        }
    }

    pub fn with_feature(mut self, feature: impl std::fmt::Display) -> Self {
        self.feature = feature.to_string();
        self
    }

    pub fn with_stage(mut self, stage: impl std::fmt::Display) -> Self {
        self.stage = stage.to_string();
        self
    }

    pub fn with_observer(
        mut self,
        observer: Arc<dyn UsageObserver>,
        tenant: TenantContext,
    ) -> Self {
        self.observer = Some((observer, tenant));
        self
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

    fn build_llm_request(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
        stream: bool,
        tools: Option<&[contracts::ToolSpec]>,
        json_mode: bool,
        max_tokens: Option<u32>,
    ) -> LlmRequest {
        let tool_defs = tools
            .map(|tools| tools.iter().map(ToolDefinition::from).collect())
            .unwrap_or_default();

        LlmRequest::new(messages.to_vec(), self.config.clone()).with_options(GenerationOptions {
            temperature,
            max_tokens,
            stream,
            json_mode,
        }).with_tools(tool_defs)
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

    async fn record_completion_success(
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

        if let Some((observer, tenant)) = &self.observer {
            let record = ChatUsageRecord {
                prompt_tokens: usage.prompt_tokens(),
                completion_tokens: usage.completion_tokens(),
                total_tokens: usage.total_tokens(),
                provider: call.provider.clone(),
                model: model.to_string(),
                feature: self.feature.clone(),
                stage: self.stage.clone(),
                session_id: None,
                document_id: None,
                request_id: None,
                trace_id: None,
            };
            observer.record_chat(tenant, &record).await;
        }
    }

    fn map_route_error(err: crate::schema::LlmError) -> anyhow::Error {
        anyhow::Error::new(err)
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
        let request = self.build_llm_request(
            messages,
            temperature,
            false,
            tools,
            json_mode,
            max_tokens,
        );

        let response = self
            .route
            .generate(request)
            .await
            .map_err(Self::map_route_error)
            .with_context(|| "Failed to complete chat request")?;

        self.record_completion_success(
            &call,
            &response.model,
            &ApiUsageRaw::from_token_counts(
                response.usage.prompt_tokens,
                response.usage.completion_tokens,
                response.usage.total_tokens,
                response.usage.cached_tokens,
            ),
            response.usage.cached_tokens as u64,
        ).await;

        Ok(response)
    }

    pub async fn complete_with_tools(
        &self,
        messages: &[ChatMessage],
        tools: &[contracts::ToolSpec],
        temperature: Option<f32>,
    ) -> anyhow::Result<LlmResponse> {
        self.complete_non_stream(messages, temperature, Some(tools), false, None)
            .await
    }

    pub async fn complete(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
    ) -> anyhow::Result<LlmResponse> {
        self.complete_non_stream(messages, temperature, None, false, None)
            .await
    }

    pub async fn complete_with_max_tokens(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
        max_tokens: u32,
    ) -> anyhow::Result<LlmResponse> {
        self.complete_non_stream(messages, temperature, None, false, Some(max_tokens))
            .await
    }

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

        if self.route.is_openai_chat() {
            return self
                .complete_stream_openai(
                    messages,
                    temperature,
                    token,
                    &call,
                    &mut on_content_delta,
                    &mut on_reasoning_delta,
                )
                .await;
        }

        self.complete_stream_events(
            messages,
            temperature,
            token,
            &call,
            &mut on_content_delta,
            &mut on_reasoning_delta,
        )
        .await
    }

    async fn complete_stream_openai(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
        token: CancellationToken,
        call: &CompletionCall,
        on_content_delta: &mut impl FnMut(&str),
        on_reasoning_delta: &mut impl FnMut(&str),
    ) -> anyhow::Result<LlmResponse> {
        let openai_route = self
            .route
            .openai_route()
            .expect("openai route should exist when is_openai_chat is true");
        let request = self.build_llm_request(messages, temperature, true, None, false, None);

        let body = openai_route
            .protocol
            .build_body(&request)
            .map_err(Self::map_route_error)?;
        let url = openai_route
            .endpoint
            .render()
            .map_err(Self::map_route_error)?;
        let mut headers = reqwest::header::HeaderMap::new();
        openai_route.auth.apply(&mut headers);

        let http_request = openai_route
            .http_client
            .post(url)
            .headers(headers)
            .header(
                reqwest::header::CONTENT_TYPE,
                reqwest::header::HeaderValue::from_static("application/json"),
            )
            .header(
                reqwest::header::ACCEPT,
                reqwest::header::HeaderValue::from_static("text/event-stream"),
            )
            .json(&body);

        let response = tokio::select! {
            res = http_request.send() => res,
            _ = token.cancelled() => anyhow::bail!("LLM request cancelled"),
        };

        let response = match response {
            Ok(response) => {
                if response.status().is_success() {
                    response
                } else {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    Self::record_call_failure(call);
                    anyhow::bail!("Chat completion stream API error {}: {}", status, body);
                }
            }
            Err(error) => {
                Self::record_call_failure(call);
                return Err(anyhow::Error::new(error))
                    .context("Failed to send chat completion stream request");
            }
        };

        let mut parser = stream_parser::ChatCompletionStreamParser::new(
            call.provider.clone(),
            call.configured_model.clone(),
        );

        let mut response = response;
        loop {
            let next_chunk = tokio::select! {
                chunk = response.chunk() => chunk.context("Failed to read chat completion stream chunk")?,
                _ = token.cancelled() => anyhow::bail!("LLM request cancelled"),
            };
            let Some(chunk) = next_chunk else {
                break;
            };

            parser.feed_chunk(&chunk, on_content_delta, on_reasoning_delta)?;
        }

        let parsed = parser.finish(on_content_delta, on_reasoning_delta)?;

        self.record_completion_success(
            call,
            &parsed.model,
            &ApiUsageRaw::from_token_counts(
                parsed.usage.prompt_tokens,
                parsed.usage.completion_tokens,
                parsed.usage.total_tokens,
                parsed.usage.cached_tokens,
            ),
            parsed.usage.cached_tokens as u64,
        ).await;

        Ok(parsed)
    }

    async fn complete_stream_events(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
        token: CancellationToken,
        call: &CompletionCall,
        on_content_delta: &mut impl FnMut(&str),
        on_reasoning_delta: &mut impl FnMut(&str),
    ) -> anyhow::Result<LlmResponse> {
        let request = self.build_llm_request(messages, temperature, true, None, false, None);
        let mut stream = self.route.stream(request);
        let mut content = String::new();
        let mut reasoning = String::new();
        let model = call.configured_model.clone();
        let mut usage = ApiUsageRaw::from_token_counts(0, 0, 0, 0);

        loop {
            let next = tokio::select! {
                event = stream.next() => event,
                _ = token.cancelled() => anyhow::bail!("LLM request cancelled"),
            };
            let Some(event) = next else {
                break;
            };
            match event.map_err(Self::map_route_error)? {
                LlmEvent::TextDelta { text, .. } => {
                    content.push_str(&text);
                    on_content_delta(&text);
                }
                LlmEvent::ReasoningDelta { text, .. } => {
                    reasoning.push_str(&text);
                    on_reasoning_delta(&text);
                }
                LlmEvent::Finish {
                    usage: Some(event_usage),
                    ..
                } => {
                    usage = ApiUsageRaw::from_token_counts(
                        event_usage.prompt_tokens,
                        event_usage.completion_tokens,
                        event_usage.total_tokens,
                        event_usage.cached_tokens,
                    );
                }
                LlmEvent::ProviderError { message, .. } => {
                    Self::record_call_failure(call);
                    anyhow::bail!(message);
                }
                _ => {}
            }
        }

        if content.is_empty() {
            if reasoning.is_empty() {
                Self::record_call_failure(call);
                anyhow::bail!("LLM stream finished without content");
            }
            content = reasoning.clone();
        }

        let response = LlmResponse {
            content,
            reasoning_content: if reasoning.is_empty() {
                None
            } else {
                Some(reasoning)
            },
            usage: LlmUsage {
                prompt_tokens: usage.prompt_tokens(),
                completion_tokens: usage.completion_tokens(),
                total_tokens: usage.total_tokens(),
                provider: call.provider.clone(),
                model: model.clone(),
                cached_tokens: usage.cached_token_count(),
            },
            model: model.clone(),
            tool_calls: None,
        };

        self.record_completion_success(call, &model, &usage, usage.cached_token_count() as u64).await;

        Ok(response)
    }
}
