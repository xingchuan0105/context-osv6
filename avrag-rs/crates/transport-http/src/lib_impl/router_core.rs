

use app_bootstrap::AppState;
use axum::{
    Json, Router,
    http::{HeaderMap, HeaderValue, StatusCode},
    routing::put,
};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use tower_http::cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;
use uuid::Uuid;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::handlers::list_workspaces,
        crate::handlers::get_workspace,
        crate::handlers::create_workspace,
        crate::handlers::update_workspace,
        crate::handlers::delete_workspace,
    ),
    components(
        schemas(
            contracts::workspaces::Workspace,
            contracts::workspaces::WorkspaceResponse,
            contracts::workspaces::WorkspaceListResponse,
            common::CreateWorkspaceRequest,
            common::UpdateWorkspaceRequest,
        )
    ),
    tags(
        (name = "workspaces", description = "Workspace management APIs")
    )
)]
struct ApiDoc;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const JWT_DEFAULT_SECRET: &str = "change-me-in-production";

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct JwtClaims {
    pub(crate) sub: String,
    pub(crate) owner_user_id: String,
    pub(crate) permissions: Vec<String>,
    jti: String,
    #[serde(default = "default_auth_version")]
    pub(crate) auth_version: i32,
    /// Optional: `"agent"` for short-lived mint tokens; absent/legacy = full session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) token_kind: Option<String>,
    pub(crate) exp: usize,
    pub(crate) iat: usize,
}

pub(crate) const TOKEN_KIND_AGENT: &str = "agent";

fn default_auth_version() -> i32 {
    1
}

// ---------------------------------------------------------------------------
// JWT helpers
// ---------------------------------------------------------------------------

fn jwt_secret() -> String {
    match std::env::var("JWT_SECRET") {
        Ok(secret) if !secret.trim().is_empty() => secret,
        _ if cfg!(any(debug_assertions, test)) => JWT_DEFAULT_SECRET.to_string(),
        _ => panic!("JWT_SECRET must be set outside debug/test builds"),
    }
}

/// API-surface product events — delegates to canonical analytics entry point.
pub(crate) async fn record_api_product_event_if_available(
    state: &AppState,
    user_id: Uuid,
    event_name: analytics::ProductEventName,
    result: analytics::ResultTag,
    metadata: serde_json::Value,
) {
    state
        .analytics_ctx_for_user(user_id)
        .record_product_event(
            event_name,
            analytics::Surface::Api,
            result,
            None,
            None,
            metadata,
        )
        .await;
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn issue_jwt(user_id: &Uuid, owner_user_id: &Uuid) -> String {
    issue_jwt_for_auth_version(user_id, owner_user_id, default_auth_version(), "user")
}

pub(crate) fn jwt_permissions_for_user_role(user_role: &str) -> Vec<String> {
    let mut permissions = vec![
        "read".to_string(),
        "write".to_string(),
        "external_network".to_string(),
    ];
    if contracts::user_role_grants_org_admin(user_role) {
        permissions.push(contracts::PERM_ADMIN.to_string());
    }
    permissions
}

#[doc(hidden)]
pub fn issue_jwt_for_auth_version(
    user_id: &Uuid,
    owner_user_id: &Uuid,
    auth_version: i32,
    user_role: &str,
) -> String {
    issue_jwt_for_auth_version_ttl(
        user_id,
        owner_user_id,
        auth_version,
        user_role,
        chrono::Duration::hours(24),
    )
}

/// Issue a user JWT with an explicit TTL (used by login and agent-token mint).
#[doc(hidden)]
pub fn issue_jwt_for_auth_version_ttl(
    user_id: &Uuid,
    owner_user_id: &Uuid,
    auth_version: i32,
    user_role: &str,
    ttl: chrono::Duration,
) -> String {
    let now = chrono::Utc::now();
    let claims = JwtClaims {
        sub: user_id.to_string(),
        owner_user_id: owner_user_id.to_string(),
        permissions: jwt_permissions_for_user_role(user_role),
        jti: Uuid::new_v4().to_string(),
        auth_version,
        token_kind: None,
        exp: (now + ttl).timestamp() as usize,
        iat: now.timestamp() as usize,
    };
    encode_jwt_claims(&claims)
}

/// Errors when minting an agent token from an existing session JWT.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentMintError {
    /// Parent is already an agent token — re-mint is not allowed.
    AgentCannotRemint,
    /// Parent JWT is already expired or has no remaining lifetime.
    ParentExpired,
}

/// Re-issue a short-lived **agent** token from a full session JWT.
///
/// - Refuses parents with `token_kind=agent` (no chain renewal).
/// - Caps child `exp` at `min(now + ttl, parent.exp)`.
/// - Sets `token_kind=agent` on the child.
pub(crate) fn reissue_agent_jwt_with_ttl(
    parent: &JwtClaims,
    ttl: chrono::Duration,
) -> Result<(String, chrono::Duration), AgentMintError> {
    if parent.token_kind.as_deref() == Some(TOKEN_KIND_AGENT) {
        return Err(AgentMintError::AgentCannotRemint);
    }
    let now = chrono::Utc::now();
    let now_ts = now.timestamp() as usize;
    if parent.exp <= now_ts {
        return Err(AgentMintError::ParentExpired);
    }
    let requested_exp = (now + ttl).timestamp() as usize;
    let child_exp = requested_exp.min(parent.exp);
    if child_exp <= now_ts {
        return Err(AgentMintError::ParentExpired);
    }
    let effective_ttl = chrono::Duration::seconds((child_exp - now_ts) as i64);
    let claims = JwtClaims {
        sub: parent.sub.clone(),
        owner_user_id: parent.owner_user_id.clone(),
        permissions: parent.permissions.clone(),
        jti: Uuid::new_v4().to_string(),
        auth_version: parent.auth_version,
        token_kind: Some(TOKEN_KIND_AGENT.to_string()),
        exp: child_exp,
        iat: now_ts,
    };
    Ok((encode_jwt_claims(&claims), effective_ttl))
}

fn encode_jwt_claims(claims: &JwtClaims) -> String {
    encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(jwt_secret().as_bytes()),
    )
    .expect("JWT encoding should not fail")
}

pub(crate) fn verify_jwt(token: &str) -> Option<JwtClaims> {
    let token_data = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(jwt_secret().as_bytes()),
        &Validation::default(),
    )
    .ok()?;
    Some(token_data.claims)
}

/// Extract Bearer token from Authorization header.
pub(crate) fn extract_bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("authorization")?
        .to_str()
        .ok()
        .and_then(|v| v.strip_prefix("Bearer "))
}

// ---------------------------------------------------------------------------
// CORS config
// ---------------------------------------------------------------------------

fn build_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(resolved_cors_origins(
            std::env::var("CORS_ALLOWED_ORIGINS").ok().as_deref(),
        )))
        .allow_methods(AllowMethods::any())
        .allow_headers(AllowHeaders::any())
}

const DEFAULT_CORS_ORIGINS: &str = "http://localhost:3000,http://127.0.0.1:3000,http://localhost:8080,http://127.0.0.1:8080,http://127.0.0.1:18080,http://localhost:18080,http://tauri.localhost,https://tauri.localhost";

fn parse_cors_origin_list(raw: &str) -> Vec<HeaderValue> {
    raw.split(',')
        .filter_map(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                trimmed.parse::<HeaderValue>().ok()
            }
        })
        .collect()
}

fn resolved_cors_origins(env_value: Option<&str>) -> Vec<HeaderValue> {
    let parsed = parse_cors_origin_list(env_value.unwrap_or(""));
    if parsed.is_empty() {
        parse_cors_origin_list(DEFAULT_CORS_ORIGINS)
    } else {
        parsed
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn build_router(state: AppState) -> Router {
    let protected_api_v1 = crate::routes::workspaces::router()
        .merge(crate::routes::chat::router())
        .merge(crate::routes::rag::router())
        .merge(crate::routes::billing::router())
        .merge(crate::routes::settings::router())
        .merge(crate::routes::desktop::router())
        .merge(crate::routes::license::router())
        .merge(crate::routes::admin::router())
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::request_context_middleware,
        ));
    let protected_auth = crate::routes::auth::protected_router().route_layer(
        axum::middleware::from_fn_with_state(state.clone(), crate::middleware::request_context_middleware),
    );
    let protected_chat_compat = crate::routes::chat::compat_router().route_layer(
        axum::middleware::from_fn_with_state(state.clone(), crate::middleware::request_context_middleware),
    );

    let protected_dev_upload = Router::new()
        .route(
            "/dev-upload/{document_id}",
            put(super::infra_handlers::dev_upload_handler)
                .layer(tower_http::limit::RequestBodyLimitLayer::new(512 * 1024 * 1024)),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::request_context_middleware,
        ));

    Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .merge(crate::routes::infra::router())
        .merge(protected_dev_upload)
        // W2 desktop official-key relay: desktop-token auth lives in the relay
        // route layer (`desktop_token_guard`), NOT the main session middleware.
        .merge(crate::routes::relay::router(&state))
        .nest("/api/auth", crate::routes::auth::public_router().merge(protected_auth))
        .nest("/api/v1", protected_api_v1)
        .nest("/api/e2e", crate::routes::e2e::router())
        .merge(protected_chat_compat)
        .with_state(state)
        .layer(axum::middleware::from_fn(crate::middleware::observability_middleware))
        .layer(TraceLayer::new_for_http())
        .layer(build_cors_layer())
        .layer(axum::extract::DefaultBodyLimit::max(64 * 1024 * 1024))
        .fallback(|| async {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "not_found",
                    "message": "Route not found",
                })),
            )
        })
}

// ---------------------------------------------------------------------------
// Auth handlers
// ---------------------------------------------------------------------------

#[cfg(test)]
mod cors_tests {
    use super::*;

    #[test]
    fn empty_cors_env_falls_back_to_localhost_list() {
        let origins = resolved_cors_origins(Some(""));
        assert!(!origins.is_empty());
        let encoded: Vec<String> = origins
            .iter()
            .map(|v| String::from_utf8_lossy(v.as_bytes()).into_owned())
            .collect();
        assert!(encoded.contains(&"http://localhost:3000".to_string()), "{encoded:?}");
        assert!(encoded.contains(&"https://tauri.localhost".to_string()), "{encoded:?}");
        assert!(!encoded.iter().any(|s| s == "*"), "{encoded:?}");
    }

    #[test]
    fn whitespace_only_cors_env_is_not_wildcard() {
        let origins = resolved_cors_origins(Some("  , , "));
        assert!(!origins.is_empty());
        assert!(origins.iter().all(|v| v.as_bytes() != b"*"));
    }

    #[test]
    fn explicit_cors_env_is_used() {
        let origins = resolved_cors_origins(Some("https://app.example.com"));
        assert_eq!(origins.len(), 1);
        assert_eq!(origins[0].as_bytes(), b"https://app.example.com");
    }
}