//! Notebook share / collab HTTP handlers.
//!
//! Business logic lives in `avrag_share::ShareService` (via `state.share()`).
//! This module only enforces auth/session guards and maps results to HTTP.

use app_bootstrap::AppState;
use axum::{
    Json,
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use super::super::{app_error_response, error_response};
use crate::auth_guard::{ensure_user_notebook_access, require_user_session};
use crate::middleware::RequestState;

#[derive(Debug, serde::Deserialize)]
pub(crate) struct CreateShareRequest {
    pub role: String,
    #[serde(default)]
    pub expires_at: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct UpdateShareSettingsBody {
    #[serde(default)]
    pub access_level: Option<String>,
    #[serde(default)]
    pub allow_download: Option<bool>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct AccessLevelBody {
    pub access_level: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct InviteMemberBody {
    pub email: String,
    pub role: String,
}

fn postgres_unavailable_response() -> Response {
    error_response(
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "Database not available",
    )
}

/// Common guard: signed-in user + notebook access + postgres share backend.
async fn require_share_session(state: &AppState, workspace_id: &str) -> Result<(), Response> {
    if let Err(error) = require_user_session(
        state.auth(),
        "this endpoint requires a signed-in user session",
    ) {
        return Err(app_error_response(error));
    }
    if let Err(error) = ensure_user_notebook_access(state, workspace_id).await {
        return Err(app_error_response(error));
    }
    if !state.postgres_configured() {
        return Err(postgres_unavailable_response());
    }
    Ok(())
}

fn parse_expires_in_secs(raw: &str) -> Option<i64> {
    let expires_at = chrono::DateTime::parse_from_rfc3339(raw).ok()?;
    let delta = expires_at
        .with_timezone(&chrono::Utc)
        .signed_duration_since(chrono::Utc::now())
        .num_seconds();
    (delta > 0).then_some(delta)
}

macro_rules! share_ok {
    ($result:expr) => {
        match $result {
            Ok(value) => (StatusCode::OK, Json(value)).into_response(),
            Err(error) => app_error_response(error),
        }
    };
}

macro_rules! share_empty_ok {
    ($result:expr) => {
        match $result {
            Ok(()) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
            Err(error) => app_error_response(error),
        }
    };
}

pub(crate) async fn create_share_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(workspace_id): Path<String>,
    Json(req): Json<CreateShareRequest>,
) -> Response {
    if let Err(response) = require_share_session(&state, &workspace_id).await {
        return response;
    }
    let expires_in_secs = req.expires_at.as_deref().and_then(parse_expires_in_secs);
    let access_level = avrag_share::AccessLevel::from_role(&req.role);
    share_ok!(
        state
            .share()
            .create_share_link(workspace_id, access_level, expires_in_secs)
            .await
    )
}

pub(crate) async fn revoke_share_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((workspace_id, token)): Path<(String, String)>,
) -> Response {
    if let Err(response) = require_share_session(&state, &workspace_id).await {
        return response;
    }
    share_empty_ok!(state.share().revoke_share_link(token).await)
}

pub(crate) async fn get_share_settings_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(workspace_id): Path<String>,
) -> Response {
    if let Err(response) = require_share_session(&state, &workspace_id).await {
        return response;
    }
    share_ok!(state.share().get_share_settings(workspace_id).await)
}

pub(crate) async fn update_share_settings_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(workspace_id): Path<String>,
    Json(req): Json<UpdateShareSettingsBody>,
) -> Response {
    if let Err(response) = require_share_session(&state, &workspace_id).await {
        return response;
    }
    share_ok!(
        state
            .share()
            .update_share_settings(workspace_id, req.access_level, req.allow_download)
            .await
    )
}

pub(crate) async fn update_access_level_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(workspace_id): Path<String>,
    Json(req): Json<AccessLevelBody>,
) -> Response {
    if let Err(response) = require_share_session(&state, &workspace_id).await {
        return response;
    }
    match state
        .share()
        .update_share_access_level(workspace_id, req.access_level)
        .await
    {
        Ok(access_level) => (
            StatusCode::OK,
            Json(serde_json::json!({ "access_level": access_level })),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn get_share_analytics_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(workspace_id): Path<String>,
) -> Response {
    if let Err(response) = require_share_session(&state, &workspace_id).await {
        return response;
    }
    share_ok!(state.share().get_share_analytics(workspace_id).await)
}

pub(crate) async fn get_share_access_logs_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(workspace_id): Path<String>,
) -> Response {
    if let Err(response) = require_share_session(&state, &workspace_id).await {
        return response;
    }
    share_ok!(state.share().get_share_access_logs(workspace_id).await)
}

pub(crate) async fn validate_share_token_handler(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Response {
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    match state.share().validate_share_token(&token).await {
        Ok(Some(workspace_id)) => (
            StatusCode::OK,
            Json(common::ShareTokenResponse {
                share_token: workspace_id,
            }),
        )
            .into_response(),
        Ok(None) => app_error_response(common::AppError::validation(
            "invalid_share_token",
            "invalid share token",
        )),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn list_members_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(workspace_id): Path<String>,
) -> Response {
    if let Err(response) = require_share_session(&state, &workspace_id).await {
        return response;
    }
    match state.share().list_share_members(workspace_id).await {
        Ok(items) => {
            let members = items
                .into_iter()
                .map(|member| contracts::share::MemberRow {
                    member_id: member.id,
                    user_id: member.user_id.unwrap_or_default(),
                    email: member.email.unwrap_or_default(),
                    role: format!("{:?}", member.access_level).to_lowercase(),
                    status: member.invite_status,
                    invited_at: member.invited_at.to_string(),
                })
                .collect();
            (
                StatusCode::OK,
                Json(contracts::share::MembersResponse { members }),
            )
                .into_response()
        }
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn invite_member_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(workspace_id): Path<String>,
    Json(req): Json<InviteMemberBody>,
) -> Response {
    if let Err(response) = require_share_session(&state, &workspace_id).await {
        return response;
    }
    let role = avrag_share::AccessLevel::from_role(&req.role);
    share_empty_ok!(
        state
            .share()
            .invite_share_member(workspace_id, req.email, role)
            .await
    )
}

pub(crate) async fn accept_member_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((workspace_id, member_id)): Path<(String, String)>,
) -> Response {
    if let Err(error) = require_user_session(
        state.auth(),
        "this endpoint requires a signed-in user session",
    ) {
        return app_error_response(error);
    }
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    share_empty_ok!(
        state
            .share()
            .accept_share_invite(workspace_id, member_id)
            .await
    )
}

pub(crate) async fn decline_member_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((workspace_id, member_id)): Path<(String, String)>,
) -> Response {
    if let Err(error) = require_user_session(
        state.auth(),
        "this endpoint requires a signed-in user session",
    ) {
        return app_error_response(error);
    }
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    share_empty_ok!(
        state
            .share()
            .decline_share_invite(workspace_id, member_id)
            .await
    )
}

pub(crate) async fn remove_member_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((workspace_id, member_id)): Path<(String, String)>,
) -> Response {
    if let Err(response) = require_share_session(&state, &workspace_id).await {
        return response;
    }
    share_empty_ok!(
        state
            .share()
            .remove_share_member(workspace_id, member_id)
            .await
    )
}
