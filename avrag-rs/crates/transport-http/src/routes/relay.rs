//! Platform official-key metered relay (2026-08-15 desktop cloud login wave, W2).
//!
//! Desktop clients without BYOK call these OpenAI-compatible routes with a
//! **desktop token** (`cos_dt_*`, minted via `POST /api/v1/desktop/tokens`).
//! The relay pins the model server-side (型号固定: chat → `AGENT_LLM_*`,
//! embeddings → `EMBEDDING_*` from `AppConfig::from_env`), calls the provider
//! with the platform api_key, streams the response through verbatim, and meters
//! actual usage through the platform `PgUsageObserver` → `debit_platform_usage`
//! (wallet list price = official × 1.5, whitelist in `wallet_pricing.rs`).
//!
//! Fail-closed on token verification; fail-open only on metering.

use app_bootstrap::AppState;
use axum::{
    Extension, Json, Router,
    body::Body,
    extract::State,
    http::{HeaderValue, Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::post,
};
use contracts::auth_runtime::{AuthContext, SubjectKind, UserId};
use serde::Deserialize;
use serde_json::json;
use std::convert::Infallible;
use uuid::Uuid;

use avrag_llm::{ChatUsageRecord, EmbeddingUsageRecord, TenantContext};

// ---------------------------------------------------------------------------
// Relay upstream config (server-side pinned; client `model` is overridden)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RelayUpstream {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub provider: String,
    pub timeout_ms: u64,
}

impl RelayUpstream {
    /// Configured only when base_url + api_key + model are all non-empty.
    fn from_model_config(config: &app_core::ModelProviderConfig) -> Option<Self> {
        let llm_config = config.to_llm_config()?;
        let model = llm_config.model.trim().to_string();
        if model.is_empty() {
            return None;
        }
        let provider = llm_config.provider_name();
        Some(Self {
            base_url: llm_config.base_url.trim_end_matches('/').to_string(),
            api_key: llm_config.api_key,
            model,
            provider,
            timeout_ms: llm_config.timeout_ms,
        })
    }
}

/// Relay routing table: which platform pool serves which OpenAI endpoint.
#[derive(Debug, Clone)]
pub struct RelayService {
    pub chat: Option<RelayUpstream>,
    pub embeddings: Option<RelayUpstream>,
    pub rerank: Option<RelayUpstream>,
    http: reqwest::Client,
}

impl RelayService {
    /// Platform pools from the canonical env config (`AppConfig::from_env`).
    pub fn from_env() -> Self {
        let config = app_core::AppConfig::from_env();
        Self {
            chat: RelayUpstream::from_model_config(&config.agent_llm),
            embeddings: RelayUpstream::from_model_config(&config.embedding),
            rerank: RelayUpstream::from_model_config(&config.rerank),
            http: reqwest::Client::new(),
        }
    }

    /// Explicit constructor (tests pin a mock upstream).
    pub fn from_upstreams(
        chat: Option<RelayUpstream>,
        embeddings: Option<RelayUpstream>,
        rerank: Option<RelayUpstream>,
    ) -> Self {
        Self {
            chat,
            embeddings,
            rerank,
            http: reqwest::Client::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Router + desktop-token guard
// ---------------------------------------------------------------------------

/// Identity carried from the guard into relay handlers.
#[derive(Debug, Clone, Copy)]
struct DesktopRelayAuth {
    user_id: Uuid,
}

pub(crate) fn router(state: &AppState) -> Router<AppState> {
    build_relay_router(state.clone(), RelayService::from_env())
}

/// Build the relay sub-router with an explicit routing table.
///
/// Auth = desktop token only (never session JWT / workspace keys); the main
/// `request_context_middleware` deliberately does not cover these routes.
#[doc(hidden)]
pub fn build_relay_router(state: AppState, service: RelayService) -> Router<AppState> {
    Router::new()
        .route("/v1/relay/chat/completions", post(relay_chat_completions))
        .route("/v1/relay/embeddings", post(relay_embeddings))
        .route("/v1/relay/rerank", post(relay_rerank))
        .layer(Extension(service))
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            desktop_token_guard,
        ))
}

async fn desktop_token_guard(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let Some(token) = crate::lib_impl::extract_bearer(req.headers()) else {
        return relay_error(
            StatusCode::UNAUTHORIZED,
            "desktop_token_required",
            "Bearer desktop token (cos_dt_*) required",
            "authentication_error",
        );
    };
    match state.desktop_api().resolve_token(token).await {
        Ok(Some(identity)) => {
            req.extensions_mut().insert(DesktopRelayAuth {
                user_id: identity.owner_user_id,
            });
            // Opportunistic last_used bump; failure must not affect the request.
            let store = state.desktop_token_store();
            let id = identity.id;
            tokio::spawn(async move {
                if let Err(error) = store.touch_last_used(id).await {
                    tracing::warn!(error = %error, token_id = %id, "desktop token last_used bump failed");
                }
            });
            next.run(req).await
        }
        Ok(None) => relay_error(
            StatusCode::UNAUTHORIZED,
            "invalid_desktop_token",
            "desktop token is unknown or revoked",
            "authentication_error",
        ),
        Err(error) => {
            // Fail closed: store outage is 5xx, never an allow.
            tracing::error!(error = %error, "desktop token resolve failed");
            relay_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "relay_auth_unavailable",
                "desktop token verification unavailable",
                "relay_error",
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn relay_chat_completions(
    State(state): State<AppState>,
    Extension(service): Extension<RelayService>,
    Extension(auth): Extension<DesktopRelayAuth>,
    body: axum::body::Bytes,
) -> Response {
    let Some(upstream) = service.chat.clone() else {
        return upstream_not_configured("chat", "AGENT_LLM");
    };
    if let Err(resp) = ensure_whitelisted(&upstream, "AGENT_LLM") {
        return resp;
    }
    if let Err(resp) = ensure_relay_balance(&state, auth.user_id).await {
        return resp;
    }

    let mut payload: serde_json::Value = match serde_json::from_slice::<serde_json::Value>(&body) {
        Ok(value) if value.is_object() => value,
        _ => {
            return relay_error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "request body must be a JSON object",
                "invalid_request_error",
            );
        }
    };
    // 型号固定：客户端 model 字段以平台 pin 为准。
    payload["model"] = json!(upstream.model);
    let wants_stream = payload
        .get("stream")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    if wants_stream {
        // Force usage in the final SSE chunk so metering sees actual tokens.
        payload["stream_options"] = json!({"include_usage": true});
    }

    let url = format!("{}/chat/completions", upstream.base_url);
    let request_id = format!("relay-{}", Uuid::new_v4());
    let response = match service
        .http
        .post(&url)
        .bearer_auth(&upstream.api_key)
        .header(header::ACCEPT, "text/event-stream")
        .timeout(std::time::Duration::from_millis(upstream.timeout_ms.max(30_000)))
        .json(&payload)
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(request_id = %request_id, error = %error, "relay upstream request failed");
            return relay_error(
                StatusCode::BAD_GATEWAY,
                "relay_upstream_request_failed",
                format!("upstream chat request failed: {error}"),
                "relay_upstream_error",
            );
        }
    };

    if !response.status().is_success() {
        return verbatim_error_response(response).await;
    }

    let observer = state.billing().usage_observer().cloned();
    let tenant = TenantContext::new(auth.user_id, auth.user_id);

    if !wants_stream {
        let bytes = response.bytes().await.unwrap_or_default();
        if let Some(usage) = parse_chat_usage(&bytes) {
            record_chat_metering(
                observer,
                &tenant,
                &upstream.provider,
                &upstream.model,
                &request_id,
                usage,
            )
            .await;
        } else {
            tracing::warn!(request_id = %request_id, "relay chat response had no usage; metering skipped (fail-open)");
        }
        return json_verbatim_response(bytes);
    }

    let provider = upstream.provider.clone();
    let model = upstream.model.clone();
    let mut scanner = SseUsageScanner::default();
    let mut upstream_response = response;
    let stream = async_stream::stream! {
        loop {
            match upstream_response.chunk().await {
                Ok(Some(bytes)) => {
                    scanner.push(&bytes);
                    yield Ok::<axum::body::Bytes, Infallible>(bytes);
                }
                Ok(None) => break,
                Err(error) => {
                    tracing::warn!(request_id = %request_id, error = %error, "relay upstream stream error; terminating");
                    break;
                }
            }
        }
        match scanner.finish() {
            Some(usage) => {
                record_chat_metering(observer, &tenant, &provider, &model, &request_id, usage).await;
            }
            None => {
                tracing::warn!(request_id = %request_id, "relay chat stream ended without usage chunk; metering skipped (fail-open)");
            }
        }
    };

    let mut response = Body::from_stream(stream).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}

async fn relay_embeddings(
    State(state): State<AppState>,
    Extension(service): Extension<RelayService>,
    Extension(auth): Extension<DesktopRelayAuth>,
    body: axum::body::Bytes,
) -> Response {
    let Some(upstream) = service.embeddings.clone() else {
        return upstream_not_configured("embeddings", "EMBEDDING");
    };
    if let Err(resp) = ensure_whitelisted(&upstream, "EMBEDDING") {
        return resp;
    }
    if let Err(resp) = ensure_relay_balance(&state, auth.user_id).await {
        return resp;
    }

    let mut payload: serde_json::Value = match serde_json::from_slice::<serde_json::Value>(&body) {
        Ok(value) if value.is_object() => value,
        _ => {
            return relay_error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "request body must be a JSON object",
                "invalid_request_error",
            );
        }
    };
    payload["model"] = json!(upstream.model);
    let estimated_tokens = estimate_embedding_tokens(&payload);

    let url = format!("{}/embeddings", upstream.base_url);
    let request_id = format!("relay-{}", Uuid::new_v4());
    let response = match service
        .http
        .post(&url)
        .bearer_auth(&upstream.api_key)
        .timeout(std::time::Duration::from_millis(upstream.timeout_ms.max(15_000)))
        .json(&payload)
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(request_id = %request_id, error = %error, "relay embeddings request failed");
            return relay_error(
                StatusCode::BAD_GATEWAY,
                "relay_upstream_request_failed",
                format!("upstream embeddings request failed: {error}"),
                "relay_upstream_error",
            );
        }
    };

    if !response.status().is_success() {
        return verbatim_error_response(response).await;
    }

    let bytes = response.bytes().await.unwrap_or_default();
    let actual_tokens = parse_embedding_usage(&bytes);
    if actual_tokens.is_some() || estimated_tokens > 0 {
        let observer = state.billing().usage_observer().cloned();
        let tenant = TenantContext::new(auth.user_id, auth.user_id);
        let record = EmbeddingUsageRecord {
            estimated_tokens,
            actual_tokens,
            provider: upstream.provider.clone(),
            model: upstream.model.clone(),
            feature: "desktop_relay".to_string(),
        };
        if let Some(observer) = observer {
            observer.record_embedding(&tenant, &record).await;
        }
    } else {
        tracing::warn!(request_id = %request_id, "relay embeddings response had no usage and request was not estimable; metering skipped (fail-open)");
    }
    json_verbatim_response(bytes)
}

/// Rerank relay (§7 R1): same shape the local `RerankerClient` speaks —
/// `POST {base}/rerank` with `{model, query, documents}`. Model pinned
/// server-side; usage metered like embeddings (providers rarely return usage
/// for rerank, so the estimate path carries it).
async fn relay_rerank(
    State(state): State<AppState>,
    Extension(service): Extension<RelayService>,
    Extension(auth): Extension<DesktopRelayAuth>,
    body: axum::body::Bytes,
) -> Response {
    let Some(upstream) = service.rerank.clone() else {
        return upstream_not_configured("rerank", "RERANK");
    };
    if let Err(resp) = ensure_whitelisted(&upstream, "RERANK") {
        return resp;
    }
    if let Err(resp) = ensure_relay_balance(&state, auth.user_id).await {
        return resp;
    }

    let mut payload: serde_json::Value = match serde_json::from_slice::<serde_json::Value>(&body) {
        Ok(value) if value.is_object() => value,
        _ => {
            return relay_error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "request body must be a JSON object",
                "invalid_request_error",
            );
        }
    };
    payload["model"] = json!(upstream.model);
    let estimated_tokens = estimate_rerank_tokens(&payload);

    let url = format!("{}/rerank", upstream.base_url);
    let request_id = format!("relay-{}", Uuid::new_v4());
    let response = match service
        .http
        .post(&url)
        .bearer_auth(&upstream.api_key)
        .timeout(std::time::Duration::from_millis(upstream.timeout_ms.max(15_000)))
        .json(&payload)
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(request_id = %request_id, error = %error, "relay rerank request failed");
            return relay_error(
                StatusCode::BAD_GATEWAY,
                "relay_upstream_request_failed",
                format!("upstream rerank request failed: {error}"),
                "relay_upstream_error",
            );
        }
    };

    if !response.status().is_success() {
        return verbatim_error_response(response).await;
    }

    let bytes = response.bytes().await.unwrap_or_default();
    let actual_tokens = parse_rerank_usage(&bytes);
    if actual_tokens.is_some() || estimated_tokens > 0 {
        let observer = state.billing().usage_observer().cloned();
        let tenant = TenantContext::new(auth.user_id, auth.user_id);
        let record = EmbeddingUsageRecord {
            estimated_tokens,
            actual_tokens,
            provider: upstream.provider.clone(),
            model: upstream.model.clone(),
            feature: "desktop_relay".to_string(),
        };
        if let Some(observer) = observer {
            observer.record_embedding(&tenant, &record).await;
        }
    } else {
        tracing::warn!(request_id = %request_id, "relay rerank response had no usage and request was not estimable; metering skipped (fail-open)");
    }
    json_verbatim_response(bytes)
}

// ---------------------------------------------------------------------------
// Preflight / metering wiring
// ---------------------------------------------------------------------------

/// Cheap balance preflight before calling upstream: empty wallet → 402-style
/// structured refusal (the cloud wallet is debited after the call by the
/// usage observer at list price).
async fn ensure_relay_balance(state: &AppState, user_id: Uuid) -> Result<(), Response> {
    let auth = AuthContext::new(UserId::from(user_id), SubjectKind::User);
    match state.billing().ensure_payer_has_wallet_balance(&auth).await {
        Ok(()) => Ok(()),
        Err(error) if error.code() == "payer_funds_required" => Err(relay_error(
            StatusCode::PAYMENT_REQUIRED,
            "payer_funds_required",
            error.message().to_string(),
            "insufficient_quota",
        )),
        Err(error) => {
            tracing::error!(error = %error, user_id = %user_id, "relay wallet preflight failed");
            Err(relay_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "wallet_preflight_failed",
                "wallet preflight failed",
                "relay_error",
            ))
        }
    }
}

/// The pinned platform model must be on the wallet price whitelist — otherwise
/// usage would be silently unmetered, so refuse with a config error instead.
fn ensure_whitelisted(upstream: &RelayUpstream, env_prefix: &str) -> Result<(), Response> {
    if avrag_billing::official_rates_for(&upstream.provider, &upstream.model).is_some() {
        return Ok(());
    }
    Err(relay_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "relay_model_not_whitelisted",
        format!(
            "pinned platform model {}/{} is not on the wallet price whitelist; \
             fix {env_prefix}_MODEL or PLATFORM_OFFICIAL_RATES_JSON",
            upstream.provider, upstream.model
        ),
        "relay_config_error",
    ))
}

/// Record actual chat usage via the platform observer (insert usage event +
/// wallet debit at list price). Observer absence (memory bootstrap) or debit
/// failure stays fail-open per UsageObserver convention.
async fn record_chat_metering(
    observer: Option<std::sync::Arc<dyn avrag_llm::UsageObserver>>,
    tenant: &TenantContext,
    provider: &str,
    model: &str,
    request_id: &str,
    usage: RelayUsage,
) {
    let Some(observer) = observer else {
        return;
    };
    let cached_tokens = usage
        .prompt_cache_hit_tokens
        .or(usage.prompt_tokens_details.map(|d| d.cached_tokens))
        .unwrap_or(0);
    let reasoning_tokens = usage
        .completion_tokens_details
        .map(|d| d.reasoning_tokens)
        .unwrap_or(0);
    let record = ChatUsageRecord {
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage
            .total_tokens
            .unwrap_or(usage.prompt_tokens + usage.completion_tokens),
        cached_tokens,
        reasoning_tokens,
        provider: provider.to_string(),
        model: model.to_string(),
        feature: "desktop_relay".to_string(),
        stage: "chat".to_string(),
        session_id: None,
        document_id: None,
        request_id: Some(request_id.to_string()),
        trace_id: None,
    };
    observer.record_chat(tenant, &record).await;
}

// ---------------------------------------------------------------------------
// Upstream response helpers
// ---------------------------------------------------------------------------

/// OpenAI-style structured error body.
fn relay_error(
    status: StatusCode,
    code: &'static str,
    message: impl Into<String>,
    err_type: &'static str,
) -> Response {
    (
        status,
        Json(json!({
            "error": {
                "code": code,
                "message": message.into(),
                "type": err_type,
            }
        })),
    )
        .into_response()
}

fn upstream_not_configured(kind: &str, env_prefix: &str) -> Response {
    relay_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "relay_upstream_not_configured",
        format!("platform {kind} upstream is not configured ({env_prefix}_API_KEY / {env_prefix}_BASE_URL)"),
        "relay_config_error",
    )
}

/// Forward an upstream error status + body verbatim.
async fn verbatim_error_response(response: reqwest::Response) -> Response {
    let status =
        StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let bytes = response.bytes().await.unwrap_or_default();
    (
        status,
        [(header::CONTENT_TYPE, "application/json")],
        bytes,
    )
        .into_response()
}

fn json_verbatim_response(bytes: axum::body::Bytes) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        bytes,
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Usage extraction (actual tokens from provider payloads)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, Deserialize)]
struct RelayUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
    #[serde(default)]
    total_tokens: Option<u32>,
    /// DeepSeek prompt-cache hit split.
    #[serde(default)]
    prompt_cache_hit_tokens: Option<u32>,
    #[serde(default)]
    prompt_tokens_details: Option<RelayPromptDetails>,
    #[serde(default)]
    completion_tokens_details: Option<RelayCompletionDetails>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct RelayPromptDetails {
    #[serde(default)]
    cached_tokens: u32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct RelayCompletionDetails {
    #[serde(default)]
    reasoning_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct RelayChatEnvelope {
    usage: Option<RelayUsage>,
}

/// Parse usage from a complete (non-stream) chat completion body.
fn parse_chat_usage(bytes: &[u8]) -> Option<RelayUsage> {
    serde_json::from_slice::<RelayChatEnvelope>(bytes)
        .ok()
        .and_then(|envelope| envelope.usage)
}

/// Parse usage from an embeddings response (`usage.total_tokens`, else prompt).
fn parse_embedding_usage(bytes: &[u8]) -> Option<u32> {
    let usage = serde_json::from_slice::<RelayChatEnvelope>(bytes)
        .ok()
        .and_then(|envelope| envelope.usage)?;
    usage
        .total_tokens
        .or(Some(usage.prompt_tokens))
        .filter(|tokens| *tokens > 0)
}

/// Rough estimate when the provider omits usage: ~4 chars per token.
fn estimate_embedding_tokens(payload: &serde_json::Value) -> u32 {
    let chars: usize = match payload.get("input") {
        Some(serde_json::Value::String(text)) => text.chars().count(),
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str())
            .map(|text| text.chars().count())
            .sum(),
        _ => return 0,
    };
    chars.div_ceil(4) as u32
}

/// Parse rerank usage: top-level `usage` envelope, else SiliconFlow-style
/// `meta.tokens.input_tokens` (their rerank responses carry usage there).
fn parse_rerank_usage(bytes: &[u8]) -> Option<u32> {
    if let Some(tokens) = parse_embedding_usage(bytes) {
        return Some(tokens);
    }
    let body = serde_json::from_slice::<serde_json::Value>(bytes).ok()?;
    body
        .get("meta")?
        .get("tokens")?
        .get("input_tokens")?
        .as_u64()
        .map(|tokens| tokens as u32)
        .filter(|tokens| *tokens > 0)
}

/// Rerank estimate (~4 chars per token) over `query` + string `documents`;
/// documents may also be `{text|image|video}` objects (multimodal shape).
fn estimate_rerank_tokens(payload: &serde_json::Value) -> u32 {
    let query_chars = payload
        .get("query")
        .and_then(|q| q.as_str())
        .map(|q| q.chars().count())
        .unwrap_or(0);
    let doc_chars: usize = match payload.get("documents") {
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str().map(|s| s.chars().count()).unwrap_or_else(|| {
                    ["text", "image", "video"]
                        .iter()
                        .filter_map(|key| item.get(key).and_then(|v| v.as_str()))
                        .map(|s| s.chars().count())
                        .sum()
                })
            })
            .sum(),
        _ => 0,
    };
    (query_chars + doc_chars).div_ceil(4) as u32
}

/// Incremental SSE scanner: retains only the incomplete-line tail between
/// chunks (O(1) memory) and captures the final `usage` chunk verbatim.
#[derive(Default)]
struct SseUsageScanner {
    tail: Vec<u8>,
    usage: Option<RelayUsage>,
}

impl SseUsageScanner {
    fn push(&mut self, chunk: &[u8]) {
        self.tail.extend_from_slice(chunk);
        while let Some(pos) = self.tail.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.tail.drain(..=pos).collect();
            self.scan_line(&line);
        }
    }

    fn scan_line(&mut self, line: &[u8]) {
        let Ok(text) = std::str::from_utf8(line) else {
            return;
        };
        let Some(data) = text.trim().strip_prefix("data:") else {
            return;
        };
        let data = data.trim();
        if data == "[DONE]" || !data.contains("\"usage\"") {
            return;
        }
        if let Ok(envelope) = serde_json::from_str::<RelayChatEnvelope>(data)
            && let Some(usage) = envelope.usage
        {
            self.usage = Some(usage);
        }
    }

    fn finish(self) -> Option<RelayUsage> {
        self.usage
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn deepseek_usage_chunk() -> String {
        concat!(
            "data: {\"id\":\"chatcmpl-1\",\"model\":\"deepseek-v4-pro\",\"choices\":[],",
            "\"usage\":{\"prompt_tokens\":100,\"completion_tokens\":20,\"total_tokens\":120,",
            "\"prompt_cache_hit_tokens\":64,\"prompt_cache_miss_tokens\":36,",
            "\"completion_tokens_details\":{\"reasoning_tokens\":8}}}\r\n\r\n",
            "data: [DONE]\r\n\r\n"
        )
        .to_string()
    }

    #[test]
    fn scanner_captures_usage_across_chunk_splits() {
        let payload = deepseek_usage_chunk();
        let bytes = payload.as_bytes();
        let mut scanner = SseUsageScanner::default();
        // Feed one byte at a time: worst-case fragmentation.
        for byte in bytes {
            scanner.push(&[*byte]);
        }
        let usage = scanner.finish().expect("usage captured");
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 20);
        assert_eq!(usage.total_tokens, Some(120));
        assert_eq!(usage.prompt_cache_hit_tokens, Some(64));
        assert_eq!(
            usage.completion_tokens_details.map(|d| d.reasoning_tokens),
            Some(8)
        );
    }

    #[test]
    fn scanner_ignores_non_data_lines_and_done_sentinel() {
        let mut scanner = SseUsageScanner::default();
        scanner.push(b": keep-alive\r\n\r\ndata: [DONE]\r\n\r\n");
        assert!(scanner.finish().is_none());
    }

    #[test]
    fn openai_style_cached_tokens_split() {
        let body = br#"{"id":"x","usage":{"prompt_tokens":50,"completion_tokens":10,"total_tokens":60,"prompt_tokens_details":{"cached_tokens":30}}}"#;
        let usage = parse_chat_usage(body).expect("usage");
        assert_eq!(usage.prompt_tokens_details.map(|d| d.cached_tokens), Some(30));
    }

    #[test]
    fn embeddings_usage_and_estimate() {
        let body = br#"{"object":"list","data":[],"usage":{"prompt_tokens":7,"total_tokens":7}}"#;
        assert_eq!(parse_embedding_usage(body), Some(7));

        let payload = json!({"model": "x", "input": ["abcdefghij", "abcd"]});
        assert_eq!(estimate_embedding_tokens(&payload), 4); // 14 chars / 4 ceil
        assert_eq!(estimate_embedding_tokens(&json!({"input": 42})), 0);
    }

    #[test]
    fn rerank_estimate_covers_query_and_document_shapes() {
        // query (4 chars) + two string docs (12) → 16/4 = 4
        let payload = json!({"model": "x", "query": "速冻", "documents": ["abcdefghij", "ab"]});
        assert_eq!(estimate_rerank_tokens(&payload), 4);
        // multimodal object documents count their text field
        let mm = json!({"query": "q", "documents": [{"text": "abcd"}, {"image": "http://x/y.png"}]});
        assert_eq!(estimate_rerank_tokens(&mm).cmp(&0), std::cmp::Ordering::Greater);
        assert_eq!(estimate_rerank_tokens(&json!({"documents": 42})), 0);
    }

    #[test]
    fn rerank_usage_reads_siliconflow_meta_tokens() {
        // SiliconFlow rerank carries usage in meta.tokens, not a usage envelope.
        let body = br#"{"id":"x","results":[],"meta":{"tokens":{"input_tokens":58,"output_tokens":0}}}"#;
        assert_eq!(parse_rerank_usage(body), Some(58));
        // usage envelope wins when both exist
        let both = br#"{"usage":{"total_tokens":7},"meta":{"tokens":{"input_tokens":58}}}"#;
        assert_eq!(parse_rerank_usage(both), Some(7));
        assert_eq!(parse_rerank_usage(br#"{"results":[]}"#), None);
    }

    #[test]
    fn rerank_model_is_whitelisted_as_embed_tier() {
        // Default platform rerank pool (SiliconFlow bge-reranker) prices under
        // the embed-tier whitelist — provider-derived or explicit both match.
        assert!(
            avrag_billing::official_rates_for("siliconflow", "Pro/BAAI/bge-reranker-v2-m3")
                .is_some()
        );
        assert!(avrag_billing::official_rates_for("custom", "BAAI/bge-reranker-v2-m3").is_some());
    }

    #[test]
    fn upstream_requires_full_config() {
        let mut config = app_core::ModelProviderConfig {
            base_url: "https://api.deepseek.com".to_string(),
            api_key: String::new(),
            model: "deepseek-v4-pro".to_string(),
            timeout_ms: 1000,
            temperature: None,
            api_style: None,
            dimensions: None,
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        };
        // No api_key → not configured (relay would 503).
        assert!(RelayUpstream::from_model_config(&config).is_none());
        config.api_key = "sk-test".to_string();
        let upstream = RelayUpstream::from_model_config(&config).expect("configured");
        assert_eq!(upstream.provider, "deepseek");
        assert_eq!(upstream.base_url, "https://api.deepseek.com");
        // Platform defaults are on the wallet whitelist (deepseek pro / dashscope embed).
        assert!(avrag_billing::official_rates_for(&upstream.provider, &upstream.model).is_some());
        config.model = " ".to_string();
        assert!(RelayUpstream::from_model_config(&config).is_none());
    }

    #[test]
    fn whitelist_refusal_shape_is_config_error() {
        let upstream = RelayUpstream {
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "sk-test".to_string(),
            model: "gpt-4o".to_string(),
            provider: "openai".to_string(),
            timeout_ms: 1000,
        };
        let response = ensure_whitelisted(&upstream, "AGENT_LLM").unwrap_err();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
