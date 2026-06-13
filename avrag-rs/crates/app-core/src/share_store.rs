use async_trait::async_trait;
use avrag_auth::AuthContext;
use common::AppError;
use uuid::Uuid;

use crate::share_domain::{
    NotebookAccessSnapshot, PublicShareChatContextSnapshot, ShareAccessLevel,
    ShareAccessLogEntry, ShareAnalyticsEntry, ShareNotebookMember,
    SharedNotebookSnapshot,
};

/// Share persistence boundary — SQL implementations live in bootstrap adapters.
#[async_trait]
pub trait ShareStorePort: Send + Sync {
    async fn query_notebook_access(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
    ) -> Result<Option<NotebookAccessSnapshot>, AppError>;

    async fn query_member_access(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<String>, AppError>;

    async fn get_share_settings(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
    ) -> Result<(String, bool, Vec<crate::share_domain::ShareTokenSnapshot>), AppError>;

    async fn list_members(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
    ) -> Result<Vec<ShareNotebookMember>, AppError>;

    async fn update_notebook_access_level(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
        access_level: &str,
    ) -> Result<(), AppError>;

    async fn update_share_settings(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
        access_level: Option<&str>,
        allow_download: Option<bool>,
    ) -> Result<(), AppError>;

    async fn create_share_token(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
        access_level: ShareAccessLevel,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<String, AppError>;

    async fn validate_token(
        &self,
        token: &str,
    ) -> Result<Option<(Uuid, ShareAccessLevel)>, AppError>;

    async fn revoke_token(
        &self,
        auth: &AuthContext,
        token: &str,
    ) -> Result<Option<Uuid>, AppError>;

    async fn get_share_analytics(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
    ) -> Result<Vec<ShareAnalyticsEntry>, AppError>;

    async fn get_share_access_logs(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
        limit: usize,
    ) -> Result<Vec<ShareAccessLogEntry>, AppError>;

    async fn load_shared_notebook(
        &self,
        token: &str,
    ) -> Result<Option<SharedNotebookSnapshot>, AppError>;

    async fn resolve_public_share_chat_context(
        &self,
        token: &str,
    ) -> Result<Option<PublicShareChatContextSnapshot>, AppError>;

    async fn invite_member(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
        email: &str,
        access_level: ShareAccessLevel,
    ) -> Result<ShareNotebookMember, AppError>;

    async fn accept_invite(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
        member_id: Uuid,
        actor_id: Uuid,
    ) -> Result<(), AppError>;

    async fn decline_invite(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
        member_id: Uuid,
        actor_id: Uuid,
    ) -> Result<(), AppError>;

    async fn add_member(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
        user_id: Uuid,
        access_level: ShareAccessLevel,
    ) -> Result<(), AppError>;

    async fn remove_member(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
        member_id: Uuid,
    ) -> Result<(), AppError>;

    async fn record_share_product_event(
        &self,
        event: analytics::ProductEvent,
    ) -> Result<(), AppError>;
}
