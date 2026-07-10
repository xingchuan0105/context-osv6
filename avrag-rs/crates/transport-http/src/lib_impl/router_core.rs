

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
    exp: usize,
    iat: usize,
}

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
    let now = chrono::Utc::now();
    let claims = JwtClaims {
        sub: user_id.to_string(),
        owner_user_id: owner_user_id.to_string(),
        permissions: jwt_permissions_for_user_role(user_role),
        jti: Uuid::new_v4().to_string(),
        auth_version,
        exp: (now + chrono::Duration::hours(24)).timestamp() as usize,
        iat: now.timestamp() as usize,
    };
    encode(
        &Header::default(),
        &claims,
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
    let allowed_origins = std::env::var("CORS_ALLOWED_ORIGINS")
        .unwrap_or_else(|_| {
            "http://localhost:3000,http://127.0.0.1:3000,http://localhost:8080,http://127.0.0.1:8080"
                .to_string()
        });
    let origins: Vec<_> = allowed_origins
        .split(',')
        .filter_map(|s| s.trim().parse::<HeaderValue>().ok())
        .collect();
    let allow_origin = if origins.is_empty() {
        AllowOrigin::any()
    } else {
        AllowOrigin::list(origins)
    };
    CorsLayer::new()
        .allow_origin(allow_origin)
        .allow_methods(AllowMethods::any())
        .allow_headers(AllowHeaders::any())
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn build_router(state: AppState) -> Router {
    let protected_api_v1 = crate::routes::workspaces::router()
        .merge(crate::routes::chat::router())
        .merge(crate::routes::rag::router())
        .merge(crate::routes::billing::router())
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
        .route("/dev-upload/{document_id}", put(super::infra_handlers::dev_upload_handler))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::request_context_middleware,
        ));

    Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .merge(crate::routes::infra::router())
        .merge(protected_dev_upload)
        .nest("/api/auth", crate::routes::auth::public_router().merge(protected_auth))
        .nest("/api/v1", protected_api_v1)
        .nest("/api/e2e", crate::routes::e2e::router())
        .merge(protected_chat_compat)
        .with_state(state)
        .layer(axum::middleware::from_fn(crate::middleware::observability_middleware))
        .layer(TraceLayer::new_for_http())
        .layer(build_cors_layer())
        .layer(axum::extract::DefaultBodyLimit::max(512 * 1024 * 1024))
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
