//! Workspace / org API-key HTTP handlers (auth only; domain logic on AppState/admin).

use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use super::super::app_error_response;
use crate::auth_guard::{ensure_user_workspace_access, forbid_api_key, require_user_admin};
use crate::middleware::RequestState;

pub(crate) async fn list_api_keys_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(workspace_id): Path<String>,
) -> Response {
    if let Err(error) = forbid_api_key(
        state.auth(),
        "API keys cannot manage other API keys; use a user session",
    ) {
        return app_error_response(error);
    }
    if let Err(error) = ensure_user_workspace_access(&state, &workspace_id).await {
        return app_error_response(error);
    }
    match state.admin_api().list_api_keys(&workspace_id).await {
        Ok(api_keys) => (
            StatusCode::OK,
            Json(common::ApiKeyListResponse { api_keys }),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn create_api_key_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(workspace_id): Path<String>,
    Json(req): Json<common::CreateApiKeyRequest>,
) -> Response {
    if let Err(error) = forbid_api_key(
        state.auth(),
        "API keys cannot manage other API keys; use a user session",
    ) {
        return app_error_response(error);
    }
    if let Err(error) = ensure_user_workspace_access(&state, &workspace_id).await {
        return app_error_response(error);
    }
    match state.admin_api().create_api_key(&workspace_id, req).await {
        Ok(resp) => (StatusCode::CREATED, Json(resp)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn create_account_api_key_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(req): Json<common::CreateApiKeyRequest>,
) -> Response {
    if let Err(error) = forbid_api_key(
        state.auth(),
        "API keys cannot manage org API keys; use a user session",
    ) {
        return app_error_response(error);
    }
    if let Err(error) = require_user_admin(state.auth()) {
        return app_error_response(error);
    }
    match state.admin_api().create_account_api_key(req).await {
        Ok(resp) => (StatusCode::CREATED, Json(resp)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn list_account_api_keys_handler(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Response {
    if let Err(error) = forbid_api_key(
        state.auth(),
        "API keys cannot manage org API keys; use a user session",
    ) {
        return app_error_response(error);
    }
    if let Err(error) = require_user_admin(state.auth()) {
        return app_error_response(error);
    }
    match state.admin_api().list_account_api_keys().await {
        Ok(api_keys) => (
            StatusCode::OK,
            Json(common::ApiKeyListResponse { api_keys }),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn revoke_account_api_key_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(key_id): Path<String>,
) -> Response {
    if let Err(error) = forbid_api_key(
        state.auth(),
        "API keys cannot manage org API keys; use a user session",
    ) {
        return app_error_response(error);
    }
    if let Err(error) = require_user_admin(state.auth()) {
        return app_error_response(error);
    }
    match state.admin_api().revoke_account_api_key(&key_id).await {
        Ok(_) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn revoke_api_key_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((workspace_id, key_id)): Path<(String, String)>,
) -> Response {
    if let Err(error) = forbid_api_key(
        state.auth(),
        "API keys cannot manage other API keys; use a user session",
    ) {
        return app_error_response(error);
    }
    if let Err(error) = ensure_user_workspace_access(&state, &workspace_id).await {
        return app_error_response(error);
    }
    match state.admin_api().revoke_api_key(&workspace_id, &key_id).await {
        Ok(_) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}
