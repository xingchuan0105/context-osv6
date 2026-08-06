use app_bootstrap::AppState;
use contracts::auth_runtime::{ActorId, AuthContext, UserId, SubjectKind};
use axum::{
    Json,
    body::{Body, to_bytes},
    extract::{Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};
use uuid::Uuid;

pub(crate) const HEADER_REQUEST_ID: &str = "x-request-id";
/// Trusted proxy account owner (`x-owner-user-id`). Personal B2C may omit and use `x-user-id`.
pub(crate) const HEADER_OWNER_USER_ID: &str = "x-owner-user-id";
pub(crate) const HEADER_USER_ID: &str = "x-user-id";
pub(crate) const HEADER_RATE_LIMIT_LIMIT: &str = "x-ratelimit-limit";
pub(crate) const HEADER_RATE_LIMIT_REMAINING: &str = "x-ratelimit-remaining";
pub(crate) const HEADER_FORWARDED_FOR: &str = "x-forwarded-for";
pub(crate) const HEADER_REAL_IP: &str = "x-real-ip";

pub(crate) const DEFAULT_RATE_LIMIT_RPM: u32 = 60;
pub(crate) const DEFAULT_EDGE_RATE_LIMIT_RPM: u32 = 120;

static LOCAL_RATE_LIMITER: LazyLock<Mutex<HashMap<String, FixedWindowCounter>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Clone)]
struct FixedWindowCounter {
    window_epoch_minute: u64,
    count: u32,
}

#[derive(Clone)]
pub(crate) struct RequestState(pub AppState);

/// Fixed-window rate limit. `window_secs=60` ≈ RPM; `86400` ≈ daily.
pub(crate) fn check_rate_limit_window(key: &str, limit: u32, window_secs: u64) -> (bool, u32, u32) {
    let window_secs = window_secs.max(1);
    let now_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / window_secs;
    let mut table = LOCAL_RATE_LIMITER.lock().unwrap();
    let counter = table.entry(key.to_string()).or_insert(FixedWindowCounter {
        window_epoch_minute: now_epoch,
        count: 0,
    });
    if counter.window_epoch_minute != now_epoch {
        counter.window_epoch_minute = now_epoch;
        counter.count = 0;
    }
    let remaining = limit.saturating_sub(counter.count);
    if counter.count < limit {
        counter.count += 1;
        (true, remaining.saturating_sub(1), limit)
    } else {
        (false, 0, limit)
    }
}

pub(crate) fn check_rate_limit(key: &str, limit_rpm: u32) -> (bool, u32, u32) {
    check_rate_limit_window(key, limit_rpm, 60)
}

pub(crate) fn extract_client_ip(headers: &HeaderMap) -> String {
    headers
        .get(HEADER_FORWARDED_FOR)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(|ip| ip.trim().to_string())
        .or_else(|| {
            headers
                .get(HEADER_REAL_IP)
                .and_then(|value| value.to_str().ok())
                .map(|ip| ip.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn retry_after_seconds_for_window() -> u64 {
    let now_epoch_sec = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    60 - (now_epoch_sec % 60)
}

pub(crate) async fn request_context_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    let is_chat_endpoint = is_chat_endpoint_path(&path);
    if is_chat_endpoint && req.method() != Method::POST {
        return next.run(req).await;
    }

    let headers = req.headers().clone();

    // Edge-layer rate limit (IP-based coarse limit before App layer)
    let edge_ip = extract_client_ip(&headers);
    let edge_key = format!("edge:{}", edge_ip);
    let edge_limit = if std::env::var("E2E_ENABLED").unwrap_or_default() == "true" {
        10_000
    } else {
        DEFAULT_EDGE_RATE_LIMIT_RPM
    };
    let (edge_allowed, _edge_remaining, edge_limit) =
        check_rate_limit_with_fallback(state.rate_limit_backend(), &edge_key, edge_limit).await;
    if !edge_allowed {
        let retry_after = retry_after_seconds_for_window();
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [
                (
                    HeaderName::from_static(HEADER_RATE_LIMIT_LIMIT),
                    edge_limit.to_string(),
                ),
                (
                    HeaderName::from_static(HEADER_RATE_LIMIT_REMAINING),
                    "0".to_string(),
                ),
                (header::RETRY_AFTER, retry_after.to_string()),
            ],
            Json(json!({
                "error": "rate_limit_exceeded",
                "message": format!("Edge rate limit of {} requests/minute exceeded", edge_limit),
                "retry_after_secs": retry_after,
            })),
        )
            .into_response();
    }

    // ADR-0010: share chat bills the **Owner**. Auth is remapped so
    // `user_id` = share owner (RLS + wallet), visitor is optional `actor_id`.
    let share_chat = share_chat_context_from_request(&state, &mut req).await;
    let visitor_auth = auth_from_bearer(&state, &headers).await.or_else(|| {
        if proxy_auth_allowed(&state) {
            auth_from_proxy_headers(&headers)
        } else {
            None
        }
    });

    let auth = match (&share_chat, visitor_auth) {
        (Some(share), visitor) => {
            if !share.allows_share_chat() {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({
                        "error": "share_not_enabled",
                        "message": "This workspace is not shared for chat.",
                    })),
                )
                    .into_response();
            }
            if !share.allows_anonymous_chat() && visitor.is_none() {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({
                        "error": "login_required",
                        "message": "This shared workspace requires sign-in before asking questions.",
                    })),
                )
                    .into_response();
            }
            // ADR-0010 §9: anonymous share chat requires Turnstile when secret is configured.
            if visitor.is_none() {
                let body_json = req
                    .extensions()
                    .get::<ShareTurnstileToken>()
                    .map(|t| serde_json::json!({ "turnstile_token": t.0 }))
                    .unwrap_or_else(|| serde_json::json!({}));
                if let Err(e) = crate::turnstile::ensure_turnstile_if_required(
                    &headers,
                    &body_json,
                    Some(edge_ip.as_str()),
                )
                .await
                {
                    return (
                        StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::BAD_REQUEST),
                        Json(json!({
                            "error": e.code(),
                            "message": e.message(),
                        })),
                    )
                        .into_response();
                }
            }
            // Per-share cost-oriented rate limit (application layer, ADR-0010 §9).
            let share_rpm: u32 = std::env::var("SHARE_CHAT_RATE_LIMIT_RPM")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30);
            // Anonymous visitors are keyed by edge IP so one crawler cannot burn
            // the whole workspace's shared nil-bucket daily quota (self-DoS).
            let visitor_part = visitor
                .as_ref()
                .and_then(|a| a.actor_id())
                .map(|a| a.into_uuid().to_string())
                .unwrap_or_else(|| format!("anon:{edge_ip}"));
            let share_key = format!("share:{}:{}", share.workspace_id, visitor_part);
            let (share_allowed, _, _) =
                check_rate_limit_with_fallback(state.rate_limit_backend(), &share_key, share_rpm)
                    .await;
            if !share_allowed {
                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(json!({
                        "error": "share_rate_limit_exceeded",
                        "message": "Share chat rate limit exceeded for this visitor.",
                    })),
                )
                    .into_response();
            }
            // ADR-0010 §9: daily question cap per visitor on a share (local fixed day window).
            let daily_cap: u32 = std::env::var("SHARE_CHAT_DAILY_LIMIT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(200);
            if daily_cap > 0 {
                let day_key = format!("{share_key}:day");
                let (day_ok, _, _) = check_rate_limit_window(&day_key, daily_cap, 86_400);
                if !day_ok {
                    return (
                        StatusCode::TOO_MANY_REQUESTS,
                        Json(json!({
                            "error": "share_daily_limit_exceeded",
                            "message": "Share chat daily question limit exceeded for this visitor.",
                        })),
                    )
                        .into_response();
                }
            }

            let mut auth = AuthContext::new(
                UserId::from(share.owner_user_id),
                SubjectKind::User,
            )
            .with_workspace_scope(share.workspace_id)
            .grant("share_chat");
            if let Some(v) = visitor {
                if let Some(actor) = v.actor_id() {
                    auth = auth.with_actor_id(actor);
                }
            }
            Some(auth)
        }
        (None, Some(auth)) => Some(auth),
        (None, None) => None,
    };

    let Some(auth) = auth else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": if is_chat_endpoint { "login_required" } else { "unauthorized" },
                "message": if is_chat_endpoint {
                    "Authentication required to chat. Shared public links may allow anonymous questions when the owner enables them."
                } else {
                    "Authentication required. Provide a Bearer token or x-owner-user-id header."
                },
            })),
        )
            .into_response();
    };

    let rate_key = format!(
        "{}:{}",
        auth.user_id().into_uuid(),
        auth.actor_id()
            .map(|actor| actor.into_uuid())
            .unwrap_or(Uuid::nil())
    );
    let mut limit_rpm = auth.rate_limit_rpm().unwrap_or(DEFAULT_RATE_LIMIT_RPM);
    if std::env::var("E2E_ENABLED").unwrap_or_default() == "true" {
        limit_rpm = 1000;
    }
    let (allowed, remaining, limit) =
        check_rate_limit_with_fallback(state.rate_limit_backend(), &rate_key, limit_rpm).await;

    let auth = if let Some(request_id) = headers
        .get(HEADER_REQUEST_ID)
        .and_then(|value| value.to_str().ok())
    {
        auth.with_request_id(request_id.to_string())
    } else {
        auth
    };

    req.extensions_mut()
        .insert(RequestState(state.with_auth(auth)));

    let is_share_request = share_chat.is_some();
    let response = next.run(req).await;

    if !allowed {
        let retry_after = retry_after_seconds_for_window();
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [
                (
                    HeaderName::from_static(HEADER_RATE_LIMIT_LIMIT),
                    limit.to_string(),
                ),
                (
                    HeaderName::from_static(HEADER_RATE_LIMIT_REMAINING),
                    "0".to_string(),
                ),
                (header::RETRY_AFTER, retry_after.to_string()),
            ],
            Json(json!({
                "error": "rate_limit_exceeded",
                "message": format!("Rate limit of {limit} requests/minute exceeded"),
                "retry_after_secs": retry_after,
            })),
        )
            .into_response();
    }

    let mut response = response;
    let response_headers = response.headers_mut();
    let _ = response_headers.insert(
        HeaderName::from_static(HEADER_RATE_LIMIT_LIMIT),
        HeaderValue::from(limit),
    );
    let _ = response_headers.insert(
        HeaderName::from_static(HEADER_RATE_LIMIT_REMAINING),
        HeaderValue::from(remaining),
    );
    // ADR-0010 §9: share chat responses must not leak token via Referer / indexing.
    if is_share_request {
        apply_share_anti_index_headers(response_headers);
    }
    response
}

/// ADR-0010 §9 headers for any public share surface (path-based or chat).
pub(crate) fn apply_share_anti_index_headers(headers: &mut axum::http::HeaderMap) {
    let _ = headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    let _ = headers.insert(
        HeaderName::from_static("x-robots-tag"),
        HeaderValue::from_static("noindex, nofollow"),
    );
}

/// True for public share HTTP paths (`/api/shared/...`, frontend `/shared/...` proxies).
#[allow(dead_code)] // used by unit tests + future path-layer middleware
pub(crate) fn req_path_is_shared(path: &str) -> bool {
    path.contains("/shared/") || path.contains("/api/shared/")
}

/// Turnstile token parsed from share chat body (optional).
#[derive(Clone, Debug)]
struct ShareTurnstileToken(String);

async fn share_chat_context_from_request(
    state: &AppState,
    req: &mut Request,
) -> Option<avrag_share::PublicShareChatContext> {
    if req.method() != Method::POST || !is_chat_endpoint_path(req.uri().path()) {
        return None;
    }

    let (parts, body) = std::mem::replace(req, Request::new(Body::empty())).into_parts();
    let body_bytes = match to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(_) => {
            *req = Request::from_parts(parts, Body::empty());
            return None;
        }
    };
    let chat_request = serde_json::from_slice::<contracts::chat::ChatRequest>(&body_bytes).ok();
    *req = Request::from_parts(parts, Body::from(body_bytes));

    let chat_request = chat_request?;
    if chat_request.source_type.as_deref() != Some("share") {
        return None;
    }
    if let Some(tok) = chat_request
        .turnstile_token
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        req.extensions_mut()
            .insert(ShareTurnstileToken(tok.to_string()));
    }
    let token = chat_request.source_token.as_deref()?;
    let ctx = state.share().resolve_public_share_chat_context(token).await?;
    if let Some(workspace_id) = chat_request.workspace_id.as_deref()
        && uuid::Uuid::parse_str(workspace_id).ok()? != ctx.workspace_id
    {
        return None;
    }

    Some(ctx)
}

fn is_chat_endpoint_path(path: &str) -> bool {
    path == "/chat" || path == "/api/v1/chat"
}

pub(crate) async fn observability_middleware(req: Request, next: Next) -> Response {
    let route = normalize_route(req.uri().path());
    let method = req.method().clone();
    let started_at = std::time::Instant::now();
    telemetry::prometheus::inc_http_inflight(route);
    let response = next.run(req).await;
    telemetry::prometheus::observe_http_request(
        route,
        method.as_str(),
        response.status().as_u16(),
        started_at.elapsed().as_secs_f64() * 1000.0,
    );
    telemetry::prometheus::dec_http_inflight(route);
    response
}

async fn check_rate_limit_with_fallback(
    backend: Option<&app_bootstrap::RedisRateLimitBackend>,
    key: &str,
    limit_rpm: u32,
) -> (bool, u32, u32) {
    if let Some(backend) = backend
        && let Ok(decision) = backend.check(key, limit_rpm).await
    {
        return (decision.allowed, decision.remaining, decision.limit);
    }

    check_rate_limit(key, limit_rpm)
}

async fn auth_from_bearer(state: &AppState, headers: &HeaderMap) -> Option<AuthContext> {
    let token = crate::lib_impl::extract_bearer(headers)?;

    if let Some(claims) = crate::lib_impl::verify_jwt(token) {
        let org_uuid = Uuid::parse_str(&claims.owner_user_id).ok()?;
        let user_uuid = Uuid::parse_str(&claims.sub).ok()?;

        if state.postgres_configured()
            && !state
                .jwt_auth_version_matches(user_uuid, org_uuid, claims.auth_version)
                .await
        {
            return None;
        }

        let mut ctx = AuthContext::new(UserId::from(org_uuid), SubjectKind::User)
            .with_actor_id(ActorId::new(user_uuid));
        for perm in &claims.permissions {
            ctx = ctx.grant(perm);
        }
        return Some(ctx);
    }

    let validated = state
        .admin_api()
        .validate_workspace_api_key(token)
        .await
        .ok()??;
    let mut ctx = AuthContext::new(validated.owner_user_id, SubjectKind::ApiKey)
        .with_actor_id(ActorId::new(validated.key_id))
        .with_rate_limit_rpm(validated.rate_limit_rpm);
    if let Some(workspace_id) = validated.workspace_id {
        ctx = ctx.with_workspace_scope(workspace_id);
    }
    for perm in validated.permissions {
        ctx = ctx.grant(perm);
    }
    Some(ctx)
}

fn proxy_auth_allowed(state: &AppState) -> bool {
    if std::env::var("E2E_ENABLED").unwrap_or_default() == "true" {
        return true;
    }
    if matches!(
        std::env::var("TRUST_PROXY_AUTH").as_deref(),
        Ok("true") | Ok("1") | Ok("yes")
    ) {
        return true;
    }
    !state.postgres_configured()
}

fn auth_from_proxy_headers(headers: &HeaderMap) -> Option<AuthContext> {
    // Account owner: x-owner-user-id, else personal account falls back to x-user-id.
    let owner_user_id = headers
        .get(HEADER_OWNER_USER_ID)
        .or_else(|| headers.get(HEADER_USER_ID))
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::parse_str(value).ok())
        .map(UserId::new)?;

    let actor = headers
        .get(HEADER_USER_ID)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::parse_str(value).ok())
        .map(ActorId::new);

    let mut ctx = AuthContext::new(owner_user_id, SubjectKind::User);
    if let Some(actor) = actor {
        ctx = ctx.with_actor_id(actor);
    } else {
        ctx = ctx.with_actor_id(ActorId::new(owner_user_id.into_uuid()));
    }
    // Support x-permissions header for testing and internal routing.
    if let Some(perms) = headers.get("x-permissions").and_then(|v| v.to_str().ok()) {
        for perm in perms.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            ctx = ctx.grant(perm);
        }
    }
    Some(ctx)
}

fn normalize_route(path: &str) -> &'static str {
    match path {
        "/health" => "/health",
        "/ready" => "/ready",
        "/metrics" => "/metrics",
        "/api/auth/register" => "/api/auth/register",
        "/api/auth/login" => "/api/auth/login",
        "/api/auth/reset/send-code" => "/api/auth/reset/send-code",
        "/api/auth/reset/verify-code" => "/api/auth/reset/verify-code",
        "/api/auth/reset/confirm" => "/api/auth/reset/confirm",
        "/api/auth/usage-limit" => "/api/auth/usage-limit",
        "/api/auth/legal-acceptance" => "/api/auth/legal-acceptance",
        "/api/auth/legal-status" => "/api/auth/legal-status",
        "/api/v1/workspaces" => "/api/v1/workspaces",
        "/api/v1/chat" => "/api/v1/chat",
        "/api/v1/mcp" => "/api/v1/mcp",
        "/api/v1/chat/sessions" => "/api/v1/chat/sessions",
        "/api/v1/chat/citations/lookup" => "/api/v1/chat/citations/lookup",
        _ if path.starts_with("/api/v1/chat/citations/assets/") => {
            "/api/v1/chat/citations/assets/:id"
        }
        "/api/v1/search" => "/api/v1/search",
        _ if path.starts_with("/api/v1/chat/sessions/") && path.ends_with("/messages") => {
            "/api/v1/chat/sessions/:id/messages"
        }
        _ if path.starts_with("/api/v1/chat/sessions/") => "/api/v1/chat/sessions/:id",
        _ if path.starts_with("/api/v1/workspaces/") => "/api/v1/workspaces/:id",
        _ if path.starts_with("/api/shared/kb/") => "/api/shared/kb/:token",
        _ if path.starts_with("/dev-upload/") => "/dev-upload/:document_id",
        _ if path.starts_with("/uploads/") => "/uploads/:document_id",
        _ if path.starts_with("/v1/workspaces/") => "/v1/workspaces/:id/chat/completions",
        _ if path.starts_with("/mcp/workspaces/") && path.ends_with("/tools/call") => {
            "/mcp/workspaces/:id/tools/call"
        }
        _ if path.starts_with("/mcp/workspaces/") => "/mcp/workspaces/:id",
        _ => "other",
    }
}
