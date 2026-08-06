//! Workspace share / collab HTTP handlers.
//!
//! Business logic lives in `avrag_share::ShareService` (via ShareApp (`state.share()`)).
//! This module only enforces auth/session guards and maps results to HTTP.

use app_bootstrap::AppState;
use axum::{
    Json,
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use super::super::{app_error_response, error_response};
use crate::auth_guard::{ensure_user_workspace_access, require_user_session};
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
    /// Daily anon visitor question cap (0 = unlimited). Omitted = no change.
    #[serde(default)]
    pub anon_question_limit: Option<i32>,
    /// Daily registered visitor cap; `null` clears to unlimited; omitted = no change.
    /// Use `member_question_limit_set` to distinguish omit vs clear.
    #[serde(default)]
    pub member_question_limit: Option<i32>,
    /// When true, apply `member_question_limit` (including null → unlimited).
    #[serde(default)]
    pub member_question_limit_set: bool,
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
    if let Err(error) = ensure_user_workspace_access(state, workspace_id).await {
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

pub(crate) async fn get_share_quota_handler(
    Extension(RequestState(state)): Extension<RequestState>,
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
    share_ok!(state.share().get_share_quota().await)
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
    let member_limit = if req.member_question_limit_set {
        Some(req.member_question_limit)
    } else {
        None
    };
    share_ok!(
        state
            .share()
            .update_share_settings(
                workspace_id,
                req.access_level,
                req.allow_download,
                req.anon_question_limit,
                member_limit,
            )
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
    let mut response = if !state.postgres_configured() {
        postgres_unavailable_response()
    } else {
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
    };
    crate::middleware::apply_share_anti_index_headers(response.headers_mut());
    response
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
    let email = req.email.clone();
    let invite_result = state
        .share()
        .invite_share_member(workspace_id.clone(), email.clone(), role)
        .await;
    if let Err(error) = invite_result {
        return app_error_response(error);
    }

    // Best-effort invite email (ADR-0010 W2 #13); UI still shows manual copy on success.
    let mail = state.password_reset_service();
    if mail.smtp_ready() {
        let base = std::env::var("AVRAG_PUBLIC_BASE_URL")
            .or_else(|_| std::env::var("PUBLIC_BASE_URL"))
            .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string());
        let accept_url = format!(
            "{}/dashboard/{workspace_id}/share?invite=1",
            base.trim_end_matches('/')
        );
        let title = state
            .workspace()
            .get_workspace(&workspace_id)
            .await
            .map(|w| w.title.clone())
            .unwrap_or_else(|| workspace_id.clone());
        if let Err(e) = mail
            .send_workspace_invite_email(&email, "workspace owner", &title, &accept_url, true)
            .await
        {
            tracing::warn!(error = %e, %email, "workspace invite email send failed");
        }
    } else {
        tracing::info!(%email, "smtp not configured; invite created without email");
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({ "ok": true })),
    )
        .into_response()
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
