use async_trait::async_trait;
use common::AppError;
use contracts::auth_runtime::AuthContext;
use uuid::Uuid;

use crate::share_domain::{
    PublicShareChatContextSnapshot, ShareAccessLevel, ShareAccessLogEntry, ShareAnalyticsEntry,
    ShareWorkspaceMember, SharedWorkspaceSnapshot, WorkspaceAccessSnapshot,
    WorkspaceShareSettingsRow,
};

/// Share persistence boundary — SQL implementations live in bootstrap adapters.
#[async_trait]
pub trait ShareStorePort: Send + Sync {
    async fn query_workspace_access(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<Option<WorkspaceAccessSnapshot>, AppError>;

    async fn query_member_access(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<String>, AppError>;

    /// When `member_user_id` is an **accepted** member of `workspace_id`, return the
    /// workspace Owner id for Owner-pays billing (ADR-0010 D2 / §4.2).
    /// `None` when the caller is not an accepted member (or is the owner).
    async fn owner_for_accepted_member(
        &self,
        workspace_id: Uuid,
        member_user_id: Uuid,
    ) -> Result<Option<Uuid>, AppError> {
        let _ = (workspace_id, member_user_id);
        Ok(None)
    }

    async fn get_share_settings(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<WorkspaceShareSettingsRow, AppError>;

    async fn list_members(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<Vec<ShareWorkspaceMember>, AppError>;

    async fn update_workspace_access_level(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        access_level: &str,
    ) -> Result<(), AppError>;

    async fn update_share_settings(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        access_level: Option<&str>,
        allow_download: Option<bool>,
        anon_question_limit: Option<i32>,
        member_question_limit: Option<Option<i32>>,
    ) -> Result<(), AppError>;

    async fn create_share_token(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        access_level: ShareAccessLevel,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<String, AppError>;

    async fn validate_token(
        &self,
        token: &str,
    ) -> Result<Option<(Uuid, ShareAccessLevel)>, AppError>;

    async fn revoke_token(&self, auth: &AuthContext, token: &str)
    -> Result<Option<Uuid>, AppError>;

    async fn get_share_analytics(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<Vec<ShareAnalyticsEntry>, AppError>;

    async fn get_share_access_logs(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        limit: usize,
    ) -> Result<Vec<ShareAccessLogEntry>, AppError>;

    async fn load_shared_workspace(
        &self,
        token: &str,
    ) -> Result<Option<SharedWorkspaceSnapshot>, AppError>;

    async fn resolve_public_share_chat_context(
        &self,
        token: &str,
    ) -> Result<Option<PublicShareChatContextSnapshot>, AppError>;

    async fn invite_member(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        email: &str,
        access_level: ShareAccessLevel,
    ) -> Result<ShareWorkspaceMember, AppError>;

    async fn accept_invite(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        member_id: Uuid,
        actor_id: Uuid,
    ) -> Result<(), AppError>;

    async fn decline_invite(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        member_id: Uuid,
        actor_id: Uuid,
    ) -> Result<(), AppError>;

    async fn add_member(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        user_id: Uuid,
        access_level: ShareAccessLevel,
    ) -> Result<(), AppError>;

    async fn remove_member(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        member_id: Uuid,
    ) -> Result<(), AppError>;

    async fn record_share_product_event(
        &self,
        event: analytics::ProductEvent,
    ) -> Result<(), AppError>;

    /// Count workspaces owned by `owner_user_id` with `share_enabled = true`.
    async fn count_share_enabled_workspaces(
        &self,
        auth: &AuthContext,
        owner_user_id: Uuid,
    ) -> Result<i64, AppError>;

    /// Max share-enabled workspaces allowed for the owner's current plan.
    async fn max_shared_workspaces_for_owner(
        &self,
        auth: &AuthContext,
        owner_user_id: Uuid,
    ) -> Result<i32, AppError>;

    /// Active subscription `plan_id` for the owner (defaults to `"free"`).
    async fn owner_plan_id(
        &self,
        auth: &AuthContext,
        owner_user_id: Uuid,
    ) -> Result<String, AppError>;

    /// Set `workspaces.share_enabled` (true occupies a plan quota slot).
    async fn set_share_enabled(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        enabled: bool,
    ) -> Result<(), AppError>;
}
