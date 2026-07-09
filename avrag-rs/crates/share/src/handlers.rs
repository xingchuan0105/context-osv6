use anyhow::Result;
use app_core::ShareStorePort;
use contracts::auth_runtime::AuthContext;
use common::{AppError, ShareTokenResponse};
use std::sync::Arc;

use crate::{
    AccessLevel, WorkspaceMember, PublicShareChatContext, ShareAccessLog, ShareAnalytics,
    ShareService, ShareSettings, SharedWorkspacePayload,
};

pub async fn handle_create_share_link(
    ctx: AuthContext,
    workspace_id: String,
    access_level: AccessLevel,
    expires_in_secs: Option<i64>,
    store: Arc<dyn ShareStorePort>,
) -> Result<ShareTokenResponse, AppError> {
    let service = ShareService::new(store);
    let token = service
        .create_share_token(&ctx, &workspace_id, access_level, expires_in_secs)
        .await
        .map_err(map_anyhow_error)?;
    Ok(ShareTokenResponse { share_token: token })
}

pub async fn handle_validate_token(
    token: &str,
    store: Arc<dyn ShareStorePort>,
) -> Result<Option<String>, AppError> {
    let service = ShareService::new(store);
    Ok(service
        .validate_token(token)
        .await
        .map_err(map_anyhow_error)?
        .map(|(workspace_id, _)| workspace_id))
}

pub async fn handle_get_shared_workspace(
    token: &str,
    store: Arc<dyn ShareStorePort>,
) -> Result<Option<SharedWorkspacePayload>, AppError> {
    let service = ShareService::new(store);
    service
        .load_shared_workspace(token)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_resolve_public_share_chat_context(
    token: &str,
    store: Arc<dyn ShareStorePort>,
) -> Result<Option<PublicShareChatContext>, AppError> {
    let service = ShareService::new(store);
    service
        .resolve_public_share_chat_context(token)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_get_share_settings(
    ctx: AuthContext,
    workspace_id: String,
    store: Arc<dyn ShareStorePort>,
) -> Result<ShareSettings, AppError> {
    let service = ShareService::new(store);
    service
        .get_share_settings(&ctx, &workspace_id)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_update_share_settings(
    ctx: AuthContext,
    workspace_id: String,
    access_level: Option<String>,
    allow_download: Option<bool>,
    store: Arc<dyn ShareStorePort>,
) -> Result<ShareSettings, AppError> {
    let service = ShareService::new(store);
    service
        .update_share_settings(&ctx, &workspace_id, access_level.as_deref(), allow_download)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_update_access_level(
    ctx: AuthContext,
    workspace_id: String,
    access_level: String,
    store: Arc<dyn ShareStorePort>,
) -> Result<String, AppError> {
    let service = ShareService::new(store);
    service
        .update_access_level(&ctx, &workspace_id, &access_level)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_revoke_share_link(
    ctx: AuthContext,
    token: String,
    store: Arc<dyn ShareStorePort>,
) -> Result<(), AppError> {
    let service = ShareService::new(store);
    service
        .revoke_token(&ctx, &token)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_invite_member(
    ctx: AuthContext,
    workspace_id: String,
    email: String,
    role: AccessLevel,
    store: Arc<dyn ShareStorePort>,
) -> Result<WorkspaceMember, AppError> {
    let service = ShareService::new(store);
    service
        .invite_member(&ctx, &workspace_id, &email, role)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_list_members(
    ctx: AuthContext,
    workspace_id: String,
    store: Arc<dyn ShareStorePort>,
) -> Result<Vec<WorkspaceMember>, AppError> {
    let service = ShareService::new(store);
    service
        .list_members(&ctx, &workspace_id)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_accept_invite(
    ctx: AuthContext,
    workspace_id: String,
    member_id: String,
    store: Arc<dyn ShareStorePort>,
) -> Result<(), AppError> {
    let service = ShareService::new(store);
    service
        .accept_invite(&ctx, &workspace_id, &member_id)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_decline_invite(
    ctx: AuthContext,
    workspace_id: String,
    member_id: String,
    store: Arc<dyn ShareStorePort>,
) -> Result<(), AppError> {
    let service = ShareService::new(store);
    service
        .decline_invite(&ctx, &workspace_id, &member_id)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_remove_member(
    ctx: AuthContext,
    workspace_id: String,
    member_id: String,
    store: Arc<dyn ShareStorePort>,
) -> Result<(), AppError> {
    let service = ShareService::new(store);
    service
        .remove_member(&ctx, &workspace_id, &member_id)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_get_share_analytics(
    ctx: AuthContext,
    workspace_id: String,
    store: Arc<dyn ShareStorePort>,
) -> Result<Vec<ShareAnalytics>, AppError> {
    let service = ShareService::new(store);
    service
        .get_share_analytics(&ctx, &workspace_id)
        .await
        .map_err(map_anyhow_error)
}

pub async fn handle_get_share_access_logs(
    ctx: AuthContext,
    workspace_id: String,
    limit: Option<usize>,
    store: Arc<dyn ShareStorePort>,
) -> Result<Vec<ShareAccessLog>, AppError> {
    let service = ShareService::new(store);
    service
        .get_share_access_logs(&ctx, &workspace_id, limit.unwrap_or(100))
        .await
        .map_err(map_anyhow_error)
}

fn map_anyhow_error(error: anyhow::Error) -> AppError {
    // If the error originated as an AppError (e.g. from the share store via
    // map_store_error), downcast it back to preserve the original variant/code/
    // http_status instead of re-classifying from a substring of the message.
    match error.downcast::<AppError>() {
        Ok(app_error) => app_error,
        Err(other) => {
            let message = other.to_string();
            if message.contains("insufficient permission") {
                AppError::unauthorized(message)
            } else if message.contains("invalid") || message.contains("parse") {
                AppError::validation("invalid_request", message)
            } else {
                AppError::internal(message)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::map_anyhow_error;
    use common::AppError;

    #[test]
    fn map_anyhow_error_recovers_preserved_app_error() {
        let original = AppError::validation("invite_not_allowed", "invite not allowed");
        let mapped = map_anyhow_error(anyhow::Error::new(original));
        assert_eq!(mapped.code(), "invite_not_allowed");
        assert_eq!(mapped.http_status(), 400);
        assert_eq!(mapped.message(), "invite not allowed");
    }

    #[test]
    fn map_anyhow_error_falls_back_to_heuristic_for_plain_anyhow() {
        let mapped = map_anyhow_error(anyhow::anyhow!("invalid input from somewhere"));
        assert_eq!(mapped.code(), "invalid_request");
        assert_eq!(mapped.http_status(), 400);
    }

    #[test]
    fn map_anyhow_error_falls_back_to_internal_for_unknown_messages() {
        let mapped = map_anyhow_error(anyhow::anyhow!("something unexpected happened"));
        assert_eq!(mapped.code(), "internal_error");
        assert_eq!(mapped.http_status(), 500);
    }
}
