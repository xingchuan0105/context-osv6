use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use tracing::{info, warn};

use app_bootstrap::AppState;

#[derive(Deserialize)]
pub(crate) struct ResetUserDataRequest {
    email: String,
}

#[derive(Deserialize)]
pub(crate) struct EnsureOrgMemberRequest {
    owner_email: String,
    member_email: String,
    password: String,
    #[serde(default)]
    full_name: String,
}

#[derive(Serialize)]
pub(crate) struct ResetUserDataResponse {
    success: bool,
    message: String,
}

pub(crate) fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route(
            "/reset-user-data",
            axum::routing::post(reset_user_data_handler),
        )
        .route(
            "/grant-admin-role",
            axum::routing::post(grant_admin_role_handler),
        )
        .route(
            "/ensure-org-member",
            axum::routing::post(ensure_org_member_handler),
        )
}

#[allow(clippy::result_large_err)]
fn validate_e2e_request(headers: &axum::http::HeaderMap, email: &str) -> Result<String, Response> {
    let node_env = env::var("NODE_ENV").unwrap_or_default();
    let e2e_enabled = env::var("E2E_ENABLED").unwrap_or_default();
    if node_env == "production" || e2e_enabled != "true" {
        warn!(
            node_env = %node_env,
            e2e_enabled = %e2e_enabled,
            "e2e request rejected: environment gate failed"
        );
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "e2e not enabled in this environment" })),
        )
            .into_response());
    }

    let expected_secret = match env::var("E2E_RESET_SECRET") {
        Ok(s) if !s.is_empty() => s,
        _ => {
            warn!("e2e request rejected: E2E_RESET_SECRET not configured");
            return Err((
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "e2e reset secret not configured" })),
            )
                .into_response());
        }
    };
    let provided_secret = headers
        .get("x-e2e-secret")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if provided_secret != expected_secret {
        warn!("e2e request rejected: secret mismatch");
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "invalid e2e secret" })),
        )
            .into_response());
    }

    let email = email.trim().to_lowercase();
    let allowed = email.starts_with("e2e-")
        || email.ends_with("@test.local")
        || email.ends_with("@example.com");
    if !allowed {
        warn!(email = %email, "e2e request rejected: account prefix gate failed");
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "account does not match allowed e2e patterns" })),
        )
            .into_response());
    }

    Ok(email)
}

async fn reset_user_data_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<ResetUserDataRequest>,
) -> Response {
    let email = match validate_e2e_request(&headers, &body.email) {
        Ok(email) => email,
        Err(response) => return response,
    };

    info!(email = %email, "e2e reset-user-data executed");

    match state.reset_e2e_user_data(&email).await {
        Ok(false) => (
            StatusCode::OK,
            Json(ResetUserDataResponse {
                success: true,
                message: "user not found, nothing to reset".to_string(),
            }),
        )
            .into_response(),
        Ok(true) => {
            info!(email = %email, "e2e reset-user-data succeeded");
            (
                StatusCode::OK,
                Json(ResetUserDataResponse {
                    success: true,
                    message: "user data reset successfully".to_string(),
                }),
            )
                .into_response()
        }
        Err(error) => {
            warn!(error = %error, email = %email, "e2e reset-user-data failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "user data reset failed" })),
            )
                .into_response()
        }
    }
}

async fn grant_admin_role_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<ResetUserDataRequest>,
) -> Response {
    let email = match validate_e2e_request(&headers, &body.email) {
        Ok(email) => email,
        Err(response) => return response,
    };

    match state.grant_e2e_admin_role(&email).await {
        Ok(()) => {
            info!(email = %email, "e2e grant-admin-role succeeded");
            (
                StatusCode::OK,
                Json(ResetUserDataResponse {
                    success: true,
                    message: "admin role granted successfully".to_string(),
                }),
            )
                .into_response()
        }
        Err(error) if error == "user not found" => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "user not found" })),
        )
            .into_response(),
        Err(error) => {
            warn!(error = %error, email = %email, "e2e grant-admin-role failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "admin role grant failed" })),
            )
                .into_response()
        }
    }
}

async fn ensure_org_member_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<EnsureOrgMemberRequest>,
) -> Response {
    let owner_email = match validate_e2e_request(&headers, &body.owner_email) {
        Ok(email) => email,
        Err(response) => return response,
    };
    let member_email = match validate_e2e_request(&headers, &body.member_email) {
        Ok(email) => email,
        Err(response) => return response,
    };
    let full_name = if body.full_name.trim().is_empty() {
        "E2E Collaborator".to_string()
    } else {
        body.full_name.trim().to_string()
    };

    match state
        .ensure_e2e_org_member(&owner_email, &member_email, &body.password, &full_name)
        .await
    {
        Ok(()) => {
            info!(owner_email = %owner_email, member_email = %member_email, "e2e ensure-org-member succeeded");
            (
                StatusCode::OK,
                Json(ResetUserDataResponse {
                    success: true,
                    message: "org member provisioned".to_string(),
                }),
            )
                .into_response()
        }
        Err(error) if error == "owner not found" => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "owner not found" })),
        )
            .into_response(),
        Err(error) => {
            warn!(
                error = %error,
                owner_email = %owner_email,
                member_email = %member_email,
                "e2e ensure-org-member failed"
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "org member provisioning failed" })),
            )
                .into_response()
        }
    }
}
