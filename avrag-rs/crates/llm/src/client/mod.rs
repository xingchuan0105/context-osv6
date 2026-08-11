pub(crate) mod rate_limit;
mod stream_parser;
mod types;

use crate::protocols::Protocol;
use crate::route::build_route_from_config;
use crate::routing::{FailureKind, LlmPoolConfig, PickError, ProviderPool};
use crate::schema::{GenerationOptions, LlmError, LlmEvent, LlmRequest, ToolDefinition};
use crate::usage_observer::{ChatUsageRecord, TenantContext, UsageObserver};
use crate::{AnyRoute, ModelProviderConfig};
use anyhow::Context;
use futures::{Stream, StreamExt};
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
    /// Optional multi-provider routing layer; when present, completions and
    /// streams go through the pool (multi-key rotation + failover) instead of
    /// the single `route`.
    pool: Option<std::sync::Arc<ProviderPool>>,
    rate_limit: ClientRateLimit,
    feature: String,
    stage: String,
    observer: Option<(Arc<dyn UsageObserver>, TenantContext)>,
    session_id: Option<uuid::Uuid>,
    request_id: Option<String>,
    /// When set, forces `enable_thinking` on every request — including pool
    /// picks that carry their own member config. Used by ReActLoop phase split
    /// (retrieve off / synthesis on).
    thinking_override: Option<bool>,
}

impl std::fmt::Debug for LlmClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmClient")
            .field("config", &self.config)
            .field("feature", &self.feature)
            .field("stage", &self.stage)
            .field("thinking_override", &self.thinking_override)
            .field("has_observer", &self.observer.is_some())
            .finish_non_exhaustive()
    }
}

impl LlmClient {
    pub fn new(config: ModelProviderConfig) -> Self {
        Self::new_with_pool(config, LlmPoolConfig::new(Vec::new()))
    }

    /// Build a client with an optional multi-provider pool. When `pool_config`
    /// has members, all completions/streams route through the pool; otherwise
    /// behavior is identical to [`Self::new`].
    pub fn new_with_pool(config: ModelProviderConfig, pool_config: LlmPoolConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(config.timeout_ms))
            .build()
            .expect("reqwest client should build");
        let transport: Arc<dyn crate::route::Transport> =
            Arc::new(crate::route::ReqwestTransport::new(client));
        Self::new_with_pool_and_transport(config, pool_config, transport)
    }

    /// Test seam: build a client whose single route and (optionally) its pool
    /// share the given transport. Injecting a fake transport lets the pool
    /// path run offline.
    pub(crate) fn new_with_pool_and_transport(
        config: ModelProviderConfig,
        pool_config: LlmPoolConfig,
        transport: Arc<dyn crate::route::Transport>,
    ) -> Self {
        let rate_limit = ClientRateLimit::from_config(&config);
        let route = build_route_from_config(&config, transport.clone());
        let pool = if pool_config.is_empty() {
            None
        } else {
            Some(std::sync::Arc::new(ProviderPool::new_with_transport(
                pool_config,
                transport,
            )))
        };
        Self {
            config,
            route,
            pool,
            rate_limit,
            feature: "agent_loop".to_string(),
            stage: String::new(),
            observer: None,
            session_id: None,
            request_id: None,
            thinking_override: None,
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

    /// Force thinking on/off for all completions on this client.
    ///
    /// Sets both `config.enable_thinking` and an override applied when building
    /// pool-pick requests (member configs keep their own credentials/models).
    /// DeepSeek maps `true` → `thinking.reasoning_effort = "max"`.
    pub fn with_enable_thinking(mut self, enable: bool) -> Self {
        self.config.enable_thinking = Some(enable);
        self.thinking_override = Some(enable);
        self
    }

    /// Attach request-level context (session_id / request_id) for usage metering.
    /// P1 (2026-07-30): flows into llm_usage_events so execute-round and
    /// cross-session cache analysis is possible (was hardcoded None).
    pub fn with_request_context(
        mut self,
        session_id: Option<uuid::Uuid>,
        request_id: Option<String>,
    ) -> Self {
        self.session_id = session_id;
        self.request_id = request_id;
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

    /// ADR-0010 BYOK: rebuild the single-route client with user credentials.
    /// Drops multi-provider pool (user key is one endpoint). Preserves metering tags.
    pub fn with_user_credentials(
        self,
        api_key: String,
        base_url: Option<String>,
        model: Option<String>,
    ) -> Self {
        let mut config = self.config.clone();
        config.api_key = api_key;
        if let Some(url) = base_url.filter(|s| !s.trim().is_empty()) {
            config.base_url = url;
        }
        if let Some(m) = model.filter(|s| !s.trim().is_empty()) {
            config.model = m;
        }
        let mut rebuilt = Self::new(config);
        rebuilt.feature = self.feature;
        rebuilt.stage = self.stage;
        rebuilt.observer = self.observer;
        rebuilt.session_id = self.session_id;
        rebuilt.request_id = self.request_id;
        rebuilt.thinking_override = self.thinking_override;
        if let Some(enable) = rebuilt.thinking_override {
            rebuilt.config.enable_thinking = Some(enable);
        }
        rebuilt
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
        self.build_llm_request_with(
            &self.config,
            messages,
            temperature,
            stream,
            tools,
            json_mode,
            max_tokens,
        )
    }

    fn build_llm_request_with(
        &self,
        config: &ModelProviderConfig,
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

        let mut effective = config.clone();
        if let Some(enable) = self.thinking_override {
            effective.enable_thinking = Some(enable);
        }

        LlmRequest::new(messages.to_vec(), effective)
            .with_options(GenerationOptions {
                temperature,
                max_tokens,
                stream,
                json_mode,
            })
            .with_tools(tool_defs)
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
        reasoning_tokens: u32,
        track_local_limits: bool,
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
        // Pool-backed calls settle limits inside the picked key's limiter;
        // only the single-route path tracks the client-level limiter here.
        if track_local_limits {
            self.rate_limit
                .record_usage(call.pre_deducted, usage.total_tokens() as usize);
        }

        if let Some((observer, tenant)) = &self.observer {
            let record = ChatUsageRecord {
                prompt_tokens: usage.prompt_tokens(),
                completion_tokens: usage.completion_tokens(),
                total_tokens: usage.total_tokens(),
                cached_tokens: usage.cached_token_count(),
                reasoning_tokens,
                provider: call.provider.clone(),
                model: model.to_string(),
                feature: self.feature.clone(),
                stage: self.stage.clone(),
                session_id: self.session_id,
                document_id: None,
                request_id: self.request_id.clone(),
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
        if let Some(pool) = &self.pool {
            return self
                .complete_non_stream_pool(pool, messages, temperature, tools, json_mode, max_tokens)
                .await;
        }
        let call = self.prepare_completion(messages)?;
        let request =
            self.build_llm_request(messages, temperature, false, tools, json_mode, max_tokens);

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
            response.usage.reasoning_tokens,
            true,
        )
        .await;

        Ok(response)
    }

    /// Non-streaming completion through the provider pool: rotates keys and
    /// fails over across members until one attempt succeeds.
    async fn complete_non_stream_pool(
        &self,
        pool: &std::sync::Arc<ProviderPool>,
        messages: &[ChatMessage],
        temperature: Option<f32>,
        tools: Option<&[contracts::ToolSpec]>,
        json_mode: bool,
        max_tokens: Option<u32>,
    ) -> anyhow::Result<LlmResponse> {
        let estimated = self.rate_limit.estimate_input_tokens(messages);
        let started_at = std::time::Instant::now();
        let owned_messages = messages.to_vec();
        let owned_tools = tools.map(|tools| tools.to_vec());

        let response = pool
            .try_each(estimated, |pick| {
                let messages = owned_messages.clone();
                let tools = owned_tools.clone();
                let client = self.clone();
                async move {
                    let request = client.build_llm_request_with(
                        &pick.config,
                        &messages,
                        temperature,
                        false,
                        tools.as_deref(),
                        json_mode,
                        max_tokens,
                    );
                    pick.route.generate(request).await
                }
            })
            .await
            .map_err(|err| anyhow::anyhow!("{err}"))?;

        let call = CompletionCall {
            started_at,
            provider: response.usage.provider.clone(),
            configured_model: response.model.clone(),
            pre_deducted: 0,
        };
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
            response.usage.reasoning_tokens,
            false,
        )
        .await;

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

    /// Session-chained completion for Responses-style providers (DashScope
    /// session cache): attaches `previous_response_id` so the provider resumes
    /// the same cached session instead of re-processing the full context.
    ///
    /// Returns the response together with its `response_id` for the next turn.
    /// Uses the fixed `self.route` (never the pool) so the chain stays on one
    /// endpoint/key; caller owns `previous_response_id` bookkeeping.
    pub async fn complete_response(
        &self,
        previous_response_id: Option<&str>,
        messages: &[ChatMessage],
        temperature: Option<f32>,
    ) -> anyhow::Result<(LlmResponse, Option<String>)> {
        let call = self.prepare_completion(messages)?;
        let request = self
            .build_llm_request(messages, temperature, false, None, false, None)
            .with_previous_response_id(previous_response_id.map(str::to_owned));

        let response = self
            .route
            .generate(request)
            .await
            .map_err(Self::map_route_error)
            .with_context(|| "Failed to complete session-chained chat request")?;

        let next_response_id = response.response_id.clone();
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
            response.usage.reasoning_tokens,
            true,
        )
        .await;

        Ok((response, next_response_id))
    }

    pub async fn complete_stream(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
        token: CancellationToken,
        mut on_content_delta: impl FnMut(&str),
        mut on_reasoning_delta: impl FnMut(&str),
    ) -> anyhow::Result<LlmResponse> {
        if let Some(pool) = &self.pool {
            return self
                .complete_stream_pool(
                    pool,
                    messages,
                    temperature,
                    token,
                    &mut on_content_delta,
                    &mut on_reasoning_delta,
                )
                .await;
        }

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
        let body_value = serde_json::to_value(&body)
            .map_err(|e| anyhow::anyhow!("failed to serialize completion body: {e}"))?;

        let transport_body = tokio::select! {
            res = openai_route.transport.post_json(&url, headers, &body_value, true) => res,
            _ = token.cancelled() => anyhow::bail!("LLM request cancelled"),
        };
        let transport_body = match transport_body {
            Ok(body) => body,
            Err(err) => {
                Self::record_call_failure(call);
                return Err(anyhow::Error::new(err))
                    .context("Failed to send chat completion stream request");
            }
        };
        let crate::route::TransportBody::Chunks(mut chunks) = transport_body else {
            anyhow::bail!("unexpected buffered response for streaming request");
        };

        let mut parser = stream_parser::ChatCompletionStreamParser::new(
            call.provider.clone(),
            call.configured_model.clone(),
        );

        loop {
            let next_chunk = tokio::select! {
                chunk = chunks.next() => chunk,
                _ = token.cancelled() => anyhow::bail!("LLM request cancelled"),
            };
            let Some(chunk) = next_chunk else {
                break;
            };
            let chunk = chunk
                .map_err(|e| anyhow::Error::new(e).context("Failed to read chat completion stream chunk"))?;

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
            parsed.usage.reasoning_tokens,
            true,
        )
        .await;

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
        let response = self
            .consume_stream_events(
                &mut stream,
                None,
                call,
                on_content_delta,
                on_reasoning_delta,
                token,
            )
            .await?;
        self.record_completion_success(
            call,
            &response.model,
            &ApiUsageRaw::from_token_counts(
                response.usage.prompt_tokens,
                response.usage.completion_tokens,
                response.usage.total_tokens,
                response.usage.cached_tokens,
            ),
            response.usage.cached_tokens as u64,
            response.usage.reasoning_tokens,
            true,
        )
        .await;
        Ok(response)
    }

    /// Streaming completion through the provider pool. Failover happens only
    /// before any content/reasoning delta is delivered (callback-fired); once
    /// delivery has started the stream runs to completion (or fails) on the
    /// picked key.
    async fn complete_stream_pool(
        &self,
        pool: &std::sync::Arc<ProviderPool>,
        messages: &[ChatMessage],
        temperature: Option<f32>,
        token: CancellationToken,
        on_content_delta: &mut impl FnMut(&str),
        on_reasoning_delta: &mut impl FnMut(&str),
    ) -> anyhow::Result<LlmResponse> {
        let estimated = self.rate_limit.estimate_input_tokens(messages);
        let mut last_error: Option<anyhow::Error> = None;
        loop {
            let pick = match pool.pick(estimated) {
                Ok(pick) => pick,
                Err(PickError::NoCapacity) => break,
            };
            let call = CompletionCall {
                started_at: std::time::Instant::now(),
                provider: pick.provider.clone(),
                configured_model: pick.model.clone(),
                pre_deducted: pick.pre_deducted,
            };
            let request = self.build_llm_request_with(
                &pick.config,
                messages,
                temperature,
                true,
                None,
                false,
                None,
            );
            let mut stream = pick.route.stream(request);
            let first = tokio::select! {
                event = stream.next() => event,
                _ = token.cancelled() => {
                    pool.report_failure(&pick, FailureKind::NotRetryable, true);
                    anyhow::bail!("LLM request cancelled");
                }
            };
            match first {
                Some(Ok(event)) => {
                    // 交付标志以「内容/推理回调真正被触发」为准：OpenAI 系协议
                    // 正常流式响应先发 TextStart/ReasoningStart 标记再发 delta，
                    // 仅看首事件会把交付后断流误判为未交付 → 错误退款 + failover
                    // 重放导致调用方收到重复前缀（2026-08-01 验收实测）。
                    // AtomicBool：Cell 会让整个 future 非 Send（调用方 spawn 需要）。
                    let delivered = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                    let delivered_c = delivered.clone();
                    let mut content_cb = |text: &str| {
                        delivered_c.store(true, std::sync::atomic::Ordering::Relaxed);
                        on_content_delta(text);
                    };
                    let delivered_r = delivered.clone();
                    let mut reasoning_cb = |text: &str| {
                        delivered_r.store(true, std::sync::atomic::Ordering::Relaxed);
                        on_reasoning_delta(text);
                    };
                    match self
                        .consume_stream_events(
                            &mut stream,
                            Some(event),
                            &call,
                            &mut content_cb,
                            &mut reasoning_cb,
                            token.clone(),
                        )
                        .await
                    {
                        Ok(response) => {
                            pool.report_success(&pick, response.usage.total_tokens as usize);
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
                                response.usage.reasoning_tokens,
                                false,
                            )
                            .await;
                            return Ok(response);
                        }
                        Err(err) if delivered.load(std::sync::atomic::Ordering::Relaxed) => {
                            // Delivery was already in progress: finish with an
                            // error and cool down the key (no refund — tokens
                            // may have been consumed).
                            Self::record_call_failure(&call);
                            pool.report_failure(&pick, FailureKind::KeyOnly, false);
                            return Err(err);
                        }
                        Err(err) => {
                            // Failed before any content was delivered. Classify
                            // (429/401/403 → key-only cooldown, not provider) so a
                            // sibling key on the same member is still tried; the
                            // raw LlmError survives inside the anyhow error.
                            Self::record_call_failure(&call);
                            let kind = err
                                .downcast_ref::<crate::schema::LlmError>()
                                .map(crate::routing::failure_kind)
                                .unwrap_or(FailureKind::Provider);
                            pool.report_failure(&pick, kind, true);
                            last_error = Some(err);
                        }
                    }
                }
                Some(Err(err)) => {
                    Self::record_call_failure(&call);
                    let kind = crate::routing::failure_kind(&err);
                    pool.report_failure(&pick, kind, true);
                    last_error = Some(Self::map_route_error(err));
                    if kind == FailureKind::NotRetryable {
                        break;
                    }
                }
                None => {
                    Self::record_call_failure(&call);
                    pool.report_failure(&pick, FailureKind::Provider, true);
                    last_error = Some(anyhow::anyhow!("LLM stream ended before any event"));
                }
            }
        }
        match last_error {
            Some(err) => Err(err),
            None => Err(anyhow::anyhow!(
                "no LLM pool candidate available (rate-limited or in cooldown)"
            )),
        }
    }

    /// Consume a routed stream from `first_event` (or from the stream head)
    /// until completion, folding deltas into the response.
    async fn consume_stream_events<S>(
        &self,
        stream: &mut S,
        first_event: Option<LlmEvent>,
        call: &CompletionCall,
        on_content_delta: &mut impl FnMut(&str),
        on_reasoning_delta: &mut impl FnMut(&str),
        token: CancellationToken,
    ) -> anyhow::Result<LlmResponse>
    where
        S: Stream<Item = Result<LlmEvent, LlmError>> + Unpin,
    {
        let mut content = String::new();
        let mut reasoning = String::new();
        let model = call.configured_model.clone();
        let mut usage = ApiUsageRaw::from_token_counts(0, 0, 0, 0);

        if let Some(event) = first_event {
            Self::apply_stream_event(
                event,
                &mut content,
                &mut reasoning,
                &mut usage,
                on_content_delta,
                on_reasoning_delta,
                call,
            )?;
        }

        loop {
            let next = tokio::select! {
                event = stream.next() => event,
                _ = token.cancelled() => anyhow::bail!("LLM request cancelled"),
            };
            let Some(event) = next else {
                break;
            };
            let event = event.map_err(Self::map_route_error)?;
            Self::apply_stream_event(
                event,
                &mut content,
                &mut reasoning,
                &mut usage,
                on_content_delta,
                on_reasoning_delta,
                call,
            )?;
        }

        if content.is_empty() {
            if reasoning.is_empty() {
                Self::record_call_failure(call);
                anyhow::bail!("LLM stream finished without content");
            }
            content = reasoning.clone();
        }

        Ok(LlmResponse {
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
                reasoning_tokens: 0,
            },
            model: model.clone(),
            tool_calls: None,
            response_id: None,
        })
    }

    fn apply_stream_event(
        event: LlmEvent,
        content: &mut String,
        reasoning: &mut String,
        usage: &mut ApiUsageRaw,
        on_content_delta: &mut impl FnMut(&str),
        on_reasoning_delta: &mut impl FnMut(&str),
        call: &CompletionCall,
    ) -> anyhow::Result<()> {
        match event {
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
                *usage = ApiUsageRaw::from_token_counts(
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
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::{LlmPoolConfig, PoolMemberConfig};
    use crate::{ApiStyle, ChatMessage};
    use std::sync::Arc;

    fn provider_config(base_url: &str) -> ModelProviderConfig {
        ModelProviderConfig {
            base_url: base_url.to_string(),
            api_key: "test-key".to_string(),
            model: "test-model".to_string(),
            timeout_ms: 5000,
            api_style: Some(ApiStyle::OpenAi),
            dimensions: None,
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        }
    }

    #[test]
    fn with_enable_thinking_sets_config_and_override() {
        let on = LlmClient::new(provider_config("https://api.deepseek.com")).with_enable_thinking(true);
        assert_eq!(on.config.enable_thinking, Some(true));
        assert_eq!(on.thinking_override, Some(true));

        let off = on.clone().with_enable_thinking(false);
        assert_eq!(off.config.enable_thinking, Some(false));
        assert_eq!(off.thinking_override, Some(false));

        // Override wins over pool pick config (pick has thinking unset).
        let pick_cfg = provider_config("https://api.deepseek.com");
        let req = off.build_llm_request_with(
            &pick_cfg,
            &[ChatMessage::user("hi")],
            None,
            false,
            None,
            false,
            None,
        );
        assert_eq!(req.config.enable_thinking, Some(false));

        let req_on = on.build_llm_request_with(
            &pick_cfg,
            &[ChatMessage::user("hi")],
            None,
            false,
            None,
            false,
            None,
        );
        assert_eq!(req_on.config.enable_thinking, Some(true));
    }

    #[test]
    fn with_user_credentials_preserves_thinking_override() {
        let client = LlmClient::new(provider_config("https://api.deepseek.com"))
            .with_enable_thinking(false)
            .with_user_credentials("new-key".into(), None, None);
        assert_eq!(client.config.api_key, "new-key");
        assert_eq!(client.config.enable_thinking, Some(false));
        assert_eq!(client.thinking_override, Some(false));
    }

    fn member(base_url: &str) -> PoolMemberConfig {
        PoolMemberConfig {
            config: provider_config(base_url),
            api_keys: vec!["test-key".to_string()],
        }
    }

    const ROLE_FRAME: &str = "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\n";
    const SUCCESS_BODY: &str = "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"B-WORLD\"}}]}\n\ndata: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":1,\"total_tokens\":2}}\n\ndata: [DONE]\n\n";

    /// Offline transport stub: routes by base-url prefix, records call URLs.
    /// Pool tests run against this instead of real axum sockets.
    #[derive(Debug, Clone)]
    enum FakeHandler {
        /// Serve an SSE body as a single chunk.
        Sse(&'static str),
        /// Serve a JSON body for a non-streaming request.
        Json(&'static str),
        /// Fail with a non-2xx status.
        Status(u16),
        /// Inspect the Authorization header; if it equals `on_auth`, fail with
        /// `error_status`, otherwise serve the success SSE body.
        AuthGate {
            on_auth: &'static str,
            error_status: u16,
        },
    }

    #[derive(Debug, Clone)]
    struct FakeTransport {
        handlers: std::sync::Arc<std::sync::Mutex<Vec<(String, FakeHandler)>>>,
        calls: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
        bodies: std::sync::Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
        headers: std::sync::Arc<std::sync::Mutex<Vec<reqwest::header::HeaderMap>>>,
    }

    impl FakeTransport {
        fn with_handlers(handlers: Vec<(String, FakeHandler)>) -> Self {
            Self {
                handlers: std::sync::Arc::new(std::sync::Mutex::new(handlers)),
                calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
                bodies: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
                headers: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            }
        }

        fn calls_for(&self, prefix: &str) -> usize {
            self.calls
                .lock()
                .unwrap()
                .iter()
                .filter(|url| url.starts_with(prefix))
                .count()
        }

        fn last_body(&self) -> serde_json::Value {
            self.bodies
                .lock()
                .unwrap()
                .last()
                .cloned()
                .unwrap_or(serde_json::Value::Null)
        }

        fn last_headers(&self) -> reqwest::header::HeaderMap {
            self.headers
                .lock()
                .unwrap()
                .last()
                .cloned()
                .unwrap_or_default()
        }
    }

    #[async_trait::async_trait]
    impl crate::route::Transport for FakeTransport {
        async fn post_json(
            &self,
            url: &str,
            headers: reqwest::header::HeaderMap,
            body: &serde_json::Value,
            _stream: bool,
        ) -> Result<crate::route::TransportBody, LlmError> {
            self.calls.lock().unwrap().push(url.to_string());
            self.bodies.lock().unwrap().push(body.clone());
            self.headers.lock().unwrap().push(headers.clone());
            let handler = self
                .handlers
                .lock()
                .unwrap()
                .iter()
                .find(|(prefix, _)| url.starts_with(prefix))
                .map(|(_, handler)| handler.clone())
                .unwrap_or(FakeHandler::Status(500));
            let sse = |body: &'static str| {
                crate::route::TransportBody::Chunks(Box::pin(futures::stream::iter(vec![
                    Ok(body.as_bytes().to_vec()),
                ])))
            };
            match handler {
                FakeHandler::Sse(body) => Ok(sse(body)),
                FakeHandler::Json(body) => {
                    let value: serde_json::Value =
                        serde_json::from_str(body).expect("fake json body parses");
                    Ok(crate::route::TransportBody::Json(value))
                }
                FakeHandler::Status(status) => Err(LlmError::Api {
                    status,
                    body: format!("Chat completion stream API error {status}: fake"),
                }),
                FakeHandler::AuthGate {
                    on_auth,
                    error_status,
                } => {
                    let bearer = headers
                        .get(reqwest::header::AUTHORIZATION)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or_default()
                        .to_string();
                    if bearer == on_auth {
                        Err(LlmError::Api {
                            status: error_status,
                            body: format!(
                                "Chat completion stream API error {error_status}: fake"
                            ),
                        })
                    } else {
                        Ok(sse(SUCCESS_BODY))
                    }
                }
            }
        }
    }

    /// 交付边界（2026-08-01 验收必修）：首事件是 TextStart 标记（role chunk），
    /// 内容 delta 已交付后流内出错——不得 failover、不得退款、不得重复输出。
    #[tokio::test]
    async fn pool_stream_failure_after_delivered_delta_does_not_failover() {
        let fake = FakeTransport::with_handlers(vec![
            (
                "fake://a".to_string(),
                FakeHandler::Sse(concat!(
                    "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\n",
                    "data: {\"choices\":[{\"delta\":{\"content\":\"A-HELLO\"}}]}\n\n",
                    "data: {not-valid-json\n\n"
                )),
            ),
            ("fake://b".to_string(), FakeHandler::Sse(SUCCESS_BODY)),
        ]);
        let client = LlmClient::new_with_pool_and_transport(
            provider_config("fake://a"),
            LlmPoolConfig::new(vec![member("fake://a"), member("fake://b")]),
            Arc::new(fake.clone()),
        );
        let mut received = String::new();
        let result = client
            .complete_stream(
                &[ChatMessage::user("hi")],
                None,
                CancellationToken::new(),
                |t| received.push_str(t),
                |_| {},
            )
            .await;

        assert!(result.is_err(), "流内错误应报错返回");
        assert_eq!(received, "A-HELLO", "内容只交付一次，不得被 failover 重放");
        assert_eq!(fake.calls_for("fake://a"), 1);
        assert_eq!(
            fake.calls_for("fake://b"),
            0,
            "交付后失败不得切到下一 member"
        );
        // KeyOnly 冷却（不退款）：下一 pick 落到 member 1。
        let pick = client.pool.as_ref().unwrap().pick(1).unwrap();
        assert_eq!(pick.member_idx, 1);
    }

    /// 交付前失败（首事件即 HTTP 错误）→ 正常 failover 到下一 member。
    #[tokio::test]
    async fn pool_stream_failure_before_first_event_fails_over() {
        let fake = FakeTransport::with_handlers(vec![
            ("fake://a".to_string(), FakeHandler::Status(500)),
            ("fake://b".to_string(), FakeHandler::Sse(SUCCESS_BODY)),
        ]);
        let client = LlmClient::new_with_pool_and_transport(
            provider_config("fake://a"),
            LlmPoolConfig::new(vec![member("fake://a"), member("fake://b")]),
            Arc::new(fake.clone()),
        );
        let mut received = String::new();
        let result = client
            .complete_stream(
                &[ChatMessage::user("hi")],
                None,
                CancellationToken::new(),
                |t| received.push_str(t),
                |_| {},
            )
            .await;

        assert!(result.is_ok(), "交付前失败应 failover 成功: {result:?}");
        assert_eq!(received, "B-WORLD");
        assert_eq!(fake.calls_for("fake://a"), 1);
    }

    /// 429 未交付 → KeyOnly 冷却(只冷却该 key),同 member 的 sibling key 仍被尝试;
    /// 若误判 Provider 会冷却整个 member 导致 NoCapacity(验收建议①)。
    #[tokio::test]
    async fn pool_stream_429_cools_key_only_sibling_key_still_tried() {
        let fake = FakeTransport::with_handlers(vec![(
            "fake://url".to_string(),
            FakeHandler::AuthGate {
                on_auth: "Bearer key-429",
                error_status: 429,
            },
        )]);
        let client = LlmClient::new_with_pool_and_transport(
            provider_config("fake://url"),
            LlmPoolConfig::new(vec![PoolMemberConfig::with_keys(
                provider_config("fake://url"),
                vec!["key-429".to_string(), "key-ok".to_string()],
            )]),
            Arc::new(fake.clone()),
        );
        let mut received = String::new();
        let result = client
            .complete_stream(
                &[ChatMessage::user("hi")],
                None,
                CancellationToken::new(),
                |t| received.push_str(t),
                |_| {},
            )
            .await;

        assert!(
            result.is_ok(),
            "429 应只冷却 key 并切 sibling key 成功: {result:?}"
        );
        assert_eq!(received, "B-WORLD");
        // 冷却落在 key 级:member 未冷却,下一次 pick 仍选同一 member 的 key-ok。
        let pick = client.pool.as_ref().unwrap().pick(1).unwrap();
        assert_eq!(pick.member_idx, 0);
    }

    const RESPONSES_JSON: &str = r#"{
        "id": "resp-session-1",
        "status": "completed",
        "model": "qwen3.7-flash",
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": "session reply"}]
        }],
        "usage": {
            "input_tokens": 10,
            "output_tokens": 5,
            "total_tokens": 15,
            "input_tokens_details": {"cached_tokens": 7, "reasoning_tokens": 0},
            "output_tokens_details": {"cached_tokens": 0, "reasoning_tokens": 0}
        }
    }"#;

    fn dashscope_config() -> ModelProviderConfig {
        ModelProviderConfig {
            base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
            api_key: "test-key".to_string(),
            model: "qwen3.7-flash".to_string(),
            timeout_ms: 5000,
            api_style: Some(ApiStyle::DashScopeResponses),
            dimensions: None,
            enable_thinking: Some(false),
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        }
    }

    /// 会话续接单测：complete_response 首轮（无 prev id）返回 response_id；
    /// 续接轮携带 previous_response_id 并带上会话缓存 header。
    #[tokio::test]
    async fn complete_response_chains_session_with_previous_id_and_header() {
        let fake = FakeTransport::with_handlers(vec![(
            "https://dashscope.aliyuncs.com".to_string(),
            FakeHandler::Json(RESPONSES_JSON),
        )]);
        let client = LlmClient::new_with_pool_and_transport(
            dashscope_config(),
            LlmPoolConfig::new(Vec::new()),
            Arc::new(fake.clone()),
        );

        // First turn: no previous id.
        let (first, next_id) = client
            .complete_response(None, &[ChatMessage::user("hi")], Some(0.1))
            .await
            .unwrap();
        assert_eq!(first.content, "session reply");
        assert_eq!(next_id.as_deref(), Some("resp-session-1"));

        let first_body = fake.last_body();
        assert_eq!(first_body["previous_response_id"], serde_json::Value::Null);
        assert_eq!(first_body["reasoning"]["effort"], "none");
        let first_headers = fake.last_headers();
        assert_eq!(
            first_headers
                .get("x-dashscope-session-cache")
                .and_then(|v| v.to_str().ok()),
            Some("enable")
        );

        // Second turn: chains the prior response id.
        let (second, next_id2) = client
            .complete_response(
                Some("resp-session-1"),
                &[ChatMessage::user("continue")],
                Some(0.1),
            )
            .await
            .unwrap();
        assert_eq!(second.content, "session reply");
        assert_eq!(next_id2.as_deref(), Some("resp-session-1"));
        let second_body = fake.last_body();
        assert_eq!(second_body["previous_response_id"], "resp-session-1");
    }
}
