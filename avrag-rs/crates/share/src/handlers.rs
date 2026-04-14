use anyhow::Result;
use avrag_auth::AuthContext;
use avrag_storage_pg::PgAppRepository;
use common::{AppError, ShareTokenResponse};
use std::sync::Arc;

use crate::{
    AccessLevel, NotebookMember, PublicShareChatContext, ShareAccessLog, ShareAnalytics, ShareService,
    ShareSettings, SharedNotebookPayload,
};

pub async fn handle_create_share_link(
    ctx: AuthContext,
    notebook_id: String,
    access_level: AccessLevel,
    expires_in_secs: Option<i64>,
    repo: Arc<PgAppRepository>,
) -> Result<axum::Json<ShareTokenResponse>, AppError> {
    let service = ShareService::new(repo);
    let token = service
        .create_share_token(&ctx, &notebook_id, access_level, expires_in_secs)
        .await
        .map_err(map_anyhow_error)?;
    Ok(axum::Json(ShareTokenResponse { share_token: token }))
}

pub async fn handle_validate_token(
    token: &str,
    repo: Arc<PgAppRepository>,
) -> Result<Option<String>, AppError> {
    let service = ShareService::new(repo);
    Ok(service
        .validate_token(token)
        .await
        .map_err(map_anyhow_error)?
        .map(|(notebook_id, _)| notebook_id))
}

pub async fn handle_get_shared_notebook(
    token: &str,
    repo: Arc<PgAppRepository>,
) -> Result<Option<SharedNotebookPayload>, AppError> {
    let service = ShareService::new(repo);
    service
        .load_shared_notebook(token)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_resolve_public_share_chat_context(
    token: &str,
    repo: Arc<PgAppRepository>,
) -> Result<Option<PublicShareChatContext>, AppError> {
    let service = ShareService::new(repo);
    service
        .resolve_public_share_chat_context(token)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_get_share_settings(
    ctx: AuthContext,
    notebook_id: String,
    repo: Arc<PgAppRepository>,
) -> Result<ShareSettings, AppError> {
    let service = ShareService::new(repo);
    service
        .get_share_settings(&ctx, &notebook_id)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_update_share_settings(
    ctx: AuthContext,
    notebook_id: String,
    access_level: Option<String>,
    allow_download: Option<bool>,
    repo: Arc<PgAppRepository>,
) -> Result<ShareSettings, AppError> {
    let service = ShareService::new(repo);
    service
        .update_share_settings(&ctx, &notebook_id, access_level.as_deref(), allow_download)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_update_access_level(
    ctx: AuthContext,
    notebook_id: String,
    access_level: String,
    repo: Arc<PgAppRepository>,
) -> Result<String, AppError> {
    let service = ShareService::new(repo);
    service
        .update_access_level(&ctx, &notebook_id, &access_level)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_revoke_share_link(
    ctx: AuthContext,
    token: String,
    repo: Arc<PgAppRepository>,
) -> Result<(), AppError> {
    let service = ShareService::new(repo);
    service
        .revoke_token(&ctx, &token)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_invite_member(
    ctx: AuthContext,
    notebook_id: String,
    email: String,
    role: AccessLevel,
    repo: Arc<PgAppRepository>,
) -> Result<NotebookMember, AppError> {
    let service = ShareService::new(repo);
    service
        .invite_member(&ctx, &notebook_id, &email, role)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_list_members(
    ctx: AuthContext,
    notebook_id: String,
    repo: Arc<PgAppRepository>,
) -> Result<Vec<NotebookMember>, AppError> {
    let service = ShareService::new(repo);
    service
        .list_members(&ctx, &notebook_id)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_accept_invite(
    ctx: AuthContext,
    notebook_id: String,
    member_id: String,
    repo: Arc<PgAppRepository>,
) -> Result<(), AppError> {
    let service = ShareService::new(repo);
    service
        .accept_invite(&ctx, &notebook_id, &member_id)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_decline_invite(
    ctx: AuthContext,
    notebook_id: String,
    member_id: String,
    repo: Arc<PgAppRepository>,
) -> Result<(), AppError> {
    let service = ShareService::new(repo);
    service
        .decline_invite(&ctx, &notebook_id, &member_id)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_remove_member(
    ctx: AuthContext,
    notebook_id: String,
    member_id: String,
    repo: Arc<PgAppRepository>,
) -> Result<(), AppError> {
    let service = ShareService::new(repo);
    service
        .remove_member(&ctx, &notebook_id, &member_id)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_get_share_analytics(
    ctx: AuthContext,
    notebook_id: String,
    repo: Arc<PgAppRepository>,
) -> Result<Vec<ShareAnalytics>, AppError> {
    let service = ShareService::new(repo);
    service
        .get_share_analytics(&ctx, &notebook_id)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_get_share_access_logs(
    ctx: AuthContext,
    notebook_id: String,
    limit: Option<usize>,
    repo: Arc<PgAppRepository>,
) -> Result<Vec<ShareAccessLog>, AppError> {
    let service = ShareService::new(repo);
    service
        .get_share_access_logs(&ctx, &notebook_id, limit.unwrap_or(100))
        .await
        .map_err(map_anyhow_error)
}

fn map_anyhow_error(error: anyhow::Error) -> AppError {
    let message = error.to_string();
    if message.contains("insufficient permission") {
        return AppError::unauthorized(message);
    }
    if message.contains("invalid") || message.contains("parse") {
        return AppError::validation("invalid_request", message);
    }
    AppError::internal(message)
}
