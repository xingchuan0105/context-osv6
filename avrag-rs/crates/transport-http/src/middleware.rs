use app::AppState;
use app::adapters::redis_rate_limiter::RedisFixedWindowRateLimiter;
use avrag_auth::{ActorId, AuthContext, OrgId, SubjectKind};
use axum::{
    Json,
    body::{Body, to_bytes},
    extract::{Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
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

pub(crate) const DEFAULT_RATE_LIMIT_RPM: u32 = 60;

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

pub(crate) async fn request_context_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    if (path == "/chat" || path == "/api/v1/chat") && req.method() != Method::POST {
        return next.run(req).await;
    }

    let headers = req.headers().clone();

    let share_chat_notebook_scope = share_chat_notebook_scope_from_request(&state, &mut req).await;
    let auth = if let Some(auth) = auth_from_bearer(&state, &headers).await {
        Some(if let Some(notebook_scope) = share_chat_notebook_scope {
            auth.with_notebook_scope(notebook_scope)
        } else {
            auth
        })
    } else if let Some(auth) = auth_from_proxy_headers(&headers) {
        Some(if let Some(notebook_scope) = share_chat_notebook_scope {
            auth.with_notebook_scope(notebook_scope)
        } else {
            auth
        })
    } else {
        None
    };

    let Some(auth) = auth else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": if path == "/api/v1/chat" { "login_required" } else { "unauthorized" },
                "message": if path == "/api/v1/chat" {
                    "Viewing shared content does not require sign-in, but asking questions does."
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
    let limit_rpm = DEFAULT_RATE_LIMIT_RPM;
    let (allowed, remaining, limit) =
        check_rate_limit_with_fallback(&state.config().redis.url, &rate_key, limit_rpm).await;

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
            ],
            Json(json!({
                "error": "rate_limit_exceeded",
                "message": format!("Rate limit of {limit} requests/minute exceeded"),
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
    if req.method() != Method::POST || req.uri().path() != "/api/v1/chat" {
        return None;
    }
    let Some(pg) = state.pg() else {
        return None;
    };

    let (parts, body) = std::mem::replace(req, Request::new(Body::empty())).into_parts();
    let body_bytes = match to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(_) => {
            *req = Request::from_parts(parts, Body::empty());
            return None;
        }
    };
    let chat_request = serde_json::from_slice::<common::ChatRequest>(&body_bytes).ok();
    *req = Request::from_parts(parts, Body::from(body_bytes));

    let chat_request = chat_request?;
    if chat_request.source_type.as_deref() != Some("share") {
        return None;
    }
    let token = chat_request.source_token.as_deref()?;
    let notebook_id = avrag_share::handle_validate_token(token, pg).await.ok()??;
    let notebook_scope = Uuid::parse_str(&notebook_id).ok()?;
    if let Some(notebook_id) = chat_request.notebook_id.as_deref()
        && uuid::Uuid::parse_str(notebook_id).ok()? != notebook_scope
    {
        return None;
    }

    Some(notebook_scope)
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
    if !redis_url.trim().is_empty() {
        if let Ok(limiter) =
            RedisFixedWindowRateLimiter::new(redis_url.to_string(), limit_rpm).await
        {
            if let Ok(decision) = limiter.check(key).await {
                return (decision.allowed, decision.remaining, decision.limit);
            }
        }
    }

    check_rate_limit(key, limit_rpm)
}

async fn auth_from_bearer(state: &AppState, headers: &HeaderMap) -> Option<AuthContext> {
    let token = crate::extract_bearer(headers)?;
    let claims = crate::verify_jwt(token)?;
    let org_uuid = Uuid::parse_str(&claims.org_id).ok()?;
    let user_uuid = Uuid::parse_str(&claims.sub).ok()?;

    if let Some(pg) = state.pg() {
        let mut tx = crate::begin_auth_admin_tx(pg.raw()).await.ok()?;
        let auth_version = sqlx::query_scalar::<_, i32>(
            "select auth_version from users where id = $1 and org_id = $2",
        )
        .bind(user_uuid)
        .bind(org_uuid)
        .fetch_optional(tx.as_mut())
        .await
        .ok()
        .flatten()?;
        let _ = tx.commit().await;

        if auth_version != claims.auth_version {
            return None;
        }
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
