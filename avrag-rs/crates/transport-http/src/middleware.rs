use app_bootstrap::AppState;
use app_core::adapters::redis_rate_limiter::RedisFixedWindowRateLimiter;
use avrag_auth::{ActorId, AuthContext, OrgId, SubjectKind};
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
pub(crate) const HEADER_ORG_ID: &str = "x-org-id";
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

pub(crate) fn check_rate_limit(key: &str, limit_rpm: u32) -> (bool, u32, u32) {
    let now_epoch_min = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 60;
    let mut table = LOCAL_RATE_LIMITER.lock().unwrap();
    let counter = table.entry(key.to_string()).or_insert(FixedWindowCounter {
        window_epoch_minute: now_epoch_min,
        count: 0,
    });
    if counter.window_epoch_minute != now_epoch_min {
        counter.window_epoch_minute = now_epoch_min;
        counter.count = 0;
    }
    let remaining = limit_rpm.saturating_sub(counter.count);
    if counter.count < limit_rpm {
        counter.count += 1;
        (true, remaining.saturating_sub(1), limit_rpm)
    } else {
        (false, 0, limit_rpm)
    }
}

fn extract_client_ip(headers: &HeaderMap) -> String {
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
        check_rate_limit_with_fallback(state.redis_url(), &edge_key, edge_limit).await;
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

    let share_chat_notebook_scope = share_chat_notebook_scope_from_request(&state, &mut req).await;
    let auth = auth_from_bearer(&state, &headers)
        .await
        .or_else(|| auth_from_proxy_headers(&headers))
        .map(|auth| {
            if let Some(notebook_scope) = share_chat_notebook_scope {
                auth.with_notebook_scope(notebook_scope)
            } else {
                auth
            }
        });

    let Some(auth) = auth else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": if is_chat_endpoint { "login_required" } else { "unauthorized" },
                "message": if is_chat_endpoint {
                    "Viewing shared content does not require sign-in, but asking questions requires sign-in."
                } else {
                    "Authentication required. Provide a Bearer token or x-org-id header."
                },
            })),
        )
            .into_response();
    };

    let rate_key = format!(
        "{}:{}",
        auth.org_id().into_uuid(),
        auth.actor_id()
            .map(|actor| actor.into_uuid())
            .unwrap_or(Uuid::nil())
    );
    let mut limit_rpm = DEFAULT_RATE_LIMIT_RPM;
    if std::env::var("E2E_ENABLED").unwrap_or_default() == "true" {
        limit_rpm = 1000;
    }
    let (allowed, remaining, limit) =
        check_rate_limit_with_fallback(state.redis_url(), &rate_key, limit_rpm).await;

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
    response
}

async fn share_chat_notebook_scope_from_request(
    state: &AppState,
    req: &mut Request,
) -> Option<Uuid> {
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
    let token = chat_request.source_token.as_deref()?;
    let notebook_scope = state.resolve_share_chat_notebook_scope(token).await?;
    if let Some(notebook_id) = chat_request.notebook_id.as_deref()
        && uuid::Uuid::parse_str(notebook_id).ok()? != notebook_scope
    {
        return None;
    }

    Some(notebook_scope)
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
    redis_url: &str,
    key: &str,
    limit_rpm: u32,
) -> (bool, u32, u32) {
    if !redis_url.trim().is_empty()
        && let Ok(limiter) =
            RedisFixedWindowRateLimiter::new(redis_url.to_string(), limit_rpm).await
        && let Ok(decision) = limiter.check(key).await
    {
        return (decision.allowed, decision.remaining, decision.limit);
    }

    check_rate_limit(key, limit_rpm)
}

async fn auth_from_bearer(state: &AppState, headers: &HeaderMap) -> Option<AuthContext> {
    let token = crate::extract_bearer(headers)?;
    let claims = crate::verify_jwt(token)?;
    let org_uuid = Uuid::parse_str(&claims.org_id).ok()?;
    let user_uuid = Uuid::parse_str(&claims.sub).ok()?;

    if state.postgres_configured()
        && !state
            .jwt_auth_version_matches(user_uuid, org_uuid, claims.auth_version)
            .await
    {
        return None;
    }

    let mut ctx = AuthContext::new(OrgId::from(org_uuid), SubjectKind::User)
        .with_actor_id(ActorId::new(user_uuid));
    for perm in &claims.permissions {
        ctx = ctx.grant(perm);
    }
    Some(ctx)
}

fn auth_from_proxy_headers(headers: &HeaderMap) -> Option<AuthContext> {
    let org_id = headers
        .get(HEADER_ORG_ID)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::parse_str(value).ok())
        .map(OrgId::new)?;

    let user_id = headers
        .get(HEADER_USER_ID)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::parse_str(value).ok())
        .map(ActorId::new);

    let mut ctx = AuthContext::new(org_id, SubjectKind::User);
    if let Some(actor) = user_id {
        ctx = ctx.with_actor_id(actor);
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
        "/api/v1/notebooks" => "/api/v1/notebooks",
        "/api/v1/chat" => "/api/v1/chat",
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
        _ if path.starts_with("/api/v1/notebooks/") => "/api/v1/notebooks/:id",
        _ if path.starts_with("/api/shared/kb/") => "/api/shared/kb/:token",
        _ if path.starts_with("/dev-upload/") => "/dev-upload/:document_id",
        _ if path.starts_with("/uploads/") => "/uploads/:document_id",
        _ if path.starts_with("/v1/notebooks/") => "/v1/notebooks/:id/chat/completions",
        _ if path.starts_with("/mcp/notebooks/") && path.ends_with("/tools/call") => {
            "/mcp/notebooks/:id/tools/call"
        }
        _ if path.starts_with("/mcp/notebooks/") => "/mcp/notebooks/:id",
        _ => "other",
    }
}
