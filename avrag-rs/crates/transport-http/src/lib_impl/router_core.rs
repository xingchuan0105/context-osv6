

use app::AppState;
use bcrypt::{DEFAULT_COST, hash, verify};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use sqlx::Row;
use serde::Deserialize;
use serde_json::json;
use tower_http::cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::warn;
use uuid::Uuid;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::handlers::list_notebooks,
        crate::handlers::get_notebook,
        crate::handlers::create_notebook,
        crate::handlers::update_notebook,
        crate::handlers::delete_notebook,
    ),
    components(
        schemas(
            common::Notebook,
            common::NotebookResponse,
            common::NotebookListResponse,
            common::CreateNotebookRequest,
            common::UpdateNotebookRequest,
        )
    ),
    tags(
        (name = "notebooks", description = "Notebook management APIs")
    )
)]
struct ApiDoc;

use auth_types::{
    AuthEnvelope, AuthPayload, AuthUserDto, ChangePasswordRequest,
    ConfirmResetPasswordRequest, LoginRequest, RegisterRequest, ResetPasswordRequest,
    ResetRequest, SendResetCodeRequest, UpdateProfileRequest, UserPreferencesPayload,
    VerifyResetCodeRequest, VerifyResetTokenRequest,
};

pub(crate) use middleware::RequestState;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const JWT_DEFAULT_SECRET: &str = "change-me-in-production";

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct JwtClaims {
    sub: String,
    org_id: String,
    permissions: Vec<String>,
    jti: String,
    #[serde(default = "default_auth_version")]
    auth_version: i32,
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

async fn record_api_product_event_if_available(
    state: &AppState,
    user_id: Uuid,
    event_name: analytics::ProductEventName,
    result: analytics::ResultTag,
    metadata: serde_json::Value,
) {
    let Some(analytics) = state.analytics() else {
        return;
    };
    let event = analytics::ProductEvent {
        event_id: Uuid::new_v4(),
        event_time: chrono::Utc::now(),
        user_id,
        session_id: None,
        notebook_id: None,
        surface: analytics::Surface::Api,
        event_name,
        result,
        request_id: state.auth().request_id().map(str::to_string),
        trace_id: None,
        client_platform: "web".to_string(),
        metadata,
    };
    if let Err(error) = analytics.record_product_event(&event).await {
        telemetry::prometheus::record_dependency_failure("analytics");
        tracing::warn!(error = %error, event_name = ?event_name, "failed to record API product event");
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn issue_jwt(user_id: &Uuid, org_id: &Uuid) -> String {
    issue_jwt_for_auth_version(user_id, org_id, default_auth_version())
}

pub(crate) fn issue_jwt_for_auth_version(
    user_id: &Uuid,
    org_id: &Uuid,
    auth_version: i32,
) -> String {
    let now = chrono::Utc::now();
    let claims = JwtClaims {
        sub: user_id.to_string(),
        org_id: org_id.to_string(),
        permissions: vec!["read".to_string(), "write".to_string()],
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
    let protected_api_v1 = routes::notebooks::router()
        .merge(routes::chat::router())
        .merge(routes::rag::router())
        .merge(routes::billing::router())
        .merge(routes::admin::router())
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::request_context_middleware,
        ));
    let protected_auth = routes::auth::protected_router().route_layer(
        axum::middleware::from_fn_with_state(state.clone(), middleware::request_context_middleware),
    );
    let protected_chat_compat = routes::chat::compat_router().route_layer(
        axum::middleware::from_fn_with_state(state.clone(), middleware::request_context_middleware),
    );

    Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .merge(routes::infra::router())
        .nest("/api/auth", routes::auth::public_router().merge(protected_auth))
        .nest("/api/v1", protected_api_v1)
        .nest("/api/e2e", routes::e2e::router())
        .merge(protected_chat_compat)
        .with_state(state)
        .layer(axum::middleware::from_fn(middleware::observability_middleware))
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
