use app_bootstrap::AppState;
use axum::{
    Json,
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::RequestState;
use super::super::{app_error_response, error_response};

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

#[derive(Debug, serde::Serialize)]
struct ApiEnvelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<ApiErrorEnvelope>,
}

#[derive(Debug, serde::Serialize)]
struct ApiErrorEnvelope {
    message: String,
}

fn postgres_unavailable_response() -> Response {
    error_response(
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "Database not available",
    )
}

pub(crate) async fn create_share_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
    Json(req): Json<CreateShareRequest>,
) -> Response {
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    let expires_in_secs = req.expires_at.as_deref().and_then(parse_expires_in_secs);
    let access_level = avrag_share::AccessLevel::from_role(&req.role);
    match state
        .create_share_link(notebook_id, access_level, expires_in_secs)
        .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn revoke_share_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((_notebook_id, token)): Path<(String, String)>,
) -> Response {
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    match state.revoke_share_link(token).await {
        Ok(()) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn get_share_settings_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
) -> Response {
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    match state.get_share_settings(notebook_id).await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn update_share_settings_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
    Json(req): Json<UpdateShareSettingsBody>,
) -> Response {
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    match state
        .update_share_settings(notebook_id, req.access_level, req.allow_download)
        .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn update_access_level_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
    Json(req): Json<AccessLevelBody>,
) -> Response {
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    match state
        .update_share_access_level(notebook_id, req.access_level)
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
    Path(notebook_id): Path<String>,
) -> Response {
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    match state.get_share_analytics(notebook_id).await {
        Ok(data) => (
            StatusCode::OK,
            Json(ApiEnvelope {
                ok: true,
                data: Some(data),
                error: None,
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<Vec<avrag_share::ShareAnalytics>> {
                ok: false,
                data: None,
                error: Some(ApiErrorEnvelope {
                    message: error.message().to_string(),
                }),
            }),
        )
            .into_response(),
    }
}

pub(crate) async fn get_share_access_logs_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
) -> Response {
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    match state.get_share_access_logs(notebook_id).await {
        Ok(data) => (
            StatusCode::OK,
            Json(ApiEnvelope {
                ok: true,
                data: Some(data),
                error: None,
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<Vec<avrag_share::ShareAccessLog>> {
                ok: false,
                data: None,
                error: Some(ApiErrorEnvelope {
                    message: error.message().to_string(),
                }),
            }),
        )
            .into_response(),
    }
}

pub(crate) async fn validate_share_token_handler(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Response {
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    match state.validate_share_token(&token).await {
        Ok(Some(notebook_id)) => (
            StatusCode::OK,
            Json(ApiEnvelope {
                ok: true,
                data: Some(common::ShareTokenResponse {
                    share_token: notebook_id,
                }),
                error: None,
            }),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::OK,
            Json(ApiEnvelope::<common::ShareTokenResponse> {
                ok: false,
                data: None,
                error: Some(ApiErrorEnvelope {
                    message: "invalid share token".to_string(),
                }),
            }),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn list_api_keys_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
) -> Response {
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
    match state.create_api_key(&notebook_id, req).await {
        Ok(resp) => (StatusCode::CREATED, Json(resp)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn revoke_api_key_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((notebook_id, key_id)): Path<(String, String)>,
) -> Response {
    match state.revoke_api_key(&notebook_id, &key_id).await {
        Ok(_) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct InviteMemberBody {
    pub email: String,
    pub role: String,
}

pub(crate) async fn list_members_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
) -> Response {
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    match state.list_share_members(notebook_id).await {
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
    Path(notebook_id): Path<String>,
    Json(req): Json<InviteMemberBody>,
) -> Response {
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    let role = avrag_share::AccessLevel::from_role(&req.role);
    match state
        .invite_share_member(notebook_id, req.email, role)
        .await
    {
        Ok(_) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn accept_member_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((notebook_id, member_id)): Path<(String, String)>,
) -> Response {
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    match state.accept_share_invite(notebook_id, member_id).await {
        Ok(()) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn decline_member_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((notebook_id, member_id)): Path<(String, String)>,
) -> Response {
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    match state.decline_share_invite(notebook_id, member_id).await {
        Ok(()) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn remove_member_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((notebook_id, member_id)): Path<(String, String)>,
) -> Response {
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    match state.remove_share_member(notebook_id, member_id).await {
        Ok(()) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn list_notifications_handler(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Response {
    match state.list_notifications(100, 0).await {
        Ok(notifications) => (
            StatusCode::OK,
            Json(common::NotificationsResponse { notifications }),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn mark_notification_read_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notification_id): Path<String>,
) -> Response {
    match state.mark_notification_read(&notification_id).await {
        Ok(_) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

fn parse_expires_in_secs(raw: &str) -> Option<i64> {
    let expires_at = chrono::DateTime::parse_from_rfc3339(raw).ok()?;
    let delta = expires_at
        .with_timezone(&chrono::Utc)
        .signed_duration_since(chrono::Utc::now())
        .num_seconds();
    (delta > 0).then_some(delta)
}
