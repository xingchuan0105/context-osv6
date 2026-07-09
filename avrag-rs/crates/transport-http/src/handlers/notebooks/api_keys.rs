//! Notebook / org API-key HTTP handlers (auth only; domain logic on AppState/admin).

use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use super::super::app_error_response;
use crate::auth_guard::{ensure_user_notebook_access, forbid_api_key, require_user_admin};
use crate::middleware::RequestState;

pub(crate) async fn list_api_keys_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
) -> Response {
    if let Err(error) = forbid_api_key(
        state.auth(),
        "API keys cannot manage other API keys; use a user session",
    ) {
        return app_error_response(error);
    }
    if let Err(error) = ensure_user_notebook_access(&state, &notebook_id).await {
        return app_error_response(error);
    }
    match state.list_api_keys(&notebook_id).await {
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
    Path(notebook_id): Path<String>,
    Json(req): Json<common::CreateApiKeyRequest>,
) -> Response {
    if let Err(error) = forbid_api_key(
        state.auth(),
        "API keys cannot manage other API keys; use a user session",
    ) {
        return app_error_response(error);
    }
    if let Err(error) = ensure_user_notebook_access(&state, &notebook_id).await {
        return app_error_response(error);
    }
    match state.create_api_key(&notebook_id, req).await {
        Ok(resp) => (StatusCode::CREATED, Json(resp)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn create_org_api_key_handler(
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
    match state.create_org_api_key(req).await {
        Ok(resp) => (StatusCode::CREATED, Json(resp)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn list_org_api_keys_handler(
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
    match state.list_org_api_keys().await {
        Ok(api_keys) => (
            StatusCode::OK,
            Json(common::ApiKeyListResponse { api_keys }),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn revoke_org_api_key_handler(
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
    match state.revoke_org_api_key(&key_id).await {
        Ok(_) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn revoke_api_key_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((notebook_id, key_id)): Path<(String, String)>,
) -> Response {
    if let Err(error) = forbid_api_key(
        state.auth(),
        "API keys cannot manage other API keys; use a user session",
    ) {
        return app_error_response(error);
    }
    if let Err(error) = ensure_user_notebook_access(&state, &notebook_id).await {
        return app_error_response(error);
    }
    match state.revoke_api_key(&notebook_id, &key_id).await {
        Ok(_) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}
