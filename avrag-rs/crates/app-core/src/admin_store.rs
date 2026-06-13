use async_trait::async_trait;
use avrag_auth::AuthContext;
use chrono::{DateTime, Utc};
use common::{
    ApiKeyRow, AppError, CreateApiKeyResponse, NotificationRow,
};
use uuid::Uuid;

use avrag_auth::OrgId;

use crate::admin_domain::{
    AdminAuditLogPage, AdminAuditLogQuery, AdminBillingOverview, AdminDegradationStatus,
    AdminFeatureFlagChangeRequest, AdminFeatureFlagEntry, AdminOrgInfo, AdminRagHealthStatus,
    AdminUsageStats, AdminUserInfo, AdminWorkerStatus,
};
use crate::domain_rows::UserProfileRow;

#[async_trait]
pub trait AdminStorePort: Send + Sync {
    async fn ensure_admin_access(&self, auth: &AuthContext) -> Result<(), AppError>;

    async fn billing_overview(&self, auth: &AuthContext) -> Result<AdminBillingOverview, AppError>;

    async fn rag_health(&self, auth: &AuthContext) -> Result<AdminRagHealthStatus, AppError>;

    async fn worker_status(&self, auth: &AuthContext) -> Result<AdminWorkerStatus, AppError>;

    async fn degradation_status(
        &self,
        auth: &AuthContext,
    ) -> Result<AdminDegradationStatus, AppError>;

    async fn list_feature_flags(
        &self,
        auth: &AuthContext,
    ) -> Result<Vec<AdminFeatureFlagEntry>, AppError>;

    async fn list_feature_flag_change_requests(
        &self,
        auth: &AuthContext,
        status: Option<&str>,
    ) -> Result<Vec<AdminFeatureFlagChangeRequest>, AppError>;

    async fn create_feature_flag_change_request(
        &self,
        auth: &AuthContext,
        key: &str,
        enabled: bool,
        reason: &str,
    ) -> Result<AdminFeatureFlagChangeRequest, AppError>;

    async fn review_feature_flag_change_request(
        &self,
        auth: &AuthContext,
        request_id: &str,
        approved: bool,
        review_note: Option<&str>,
    ) -> Result<AdminFeatureFlagChangeRequest, AppError>;

    async fn get_user_profile(
        &self,
        auth: &AuthContext,
        user_id: Uuid,
    ) -> Result<Option<UserProfileRow>, AppError>;

    async fn upsert_user_profile(
        &self,
        auth: &AuthContext,
        profile: &UserProfileRow,
    ) -> Result<(), AppError>;

    async fn list_api_keys(
        &self,
        auth: &AuthContext,
        notebook_id: Option<Uuid>,
    ) -> Result<Vec<ApiKeyRow>, AppError>;

    async fn create_api_key(
        &self,
        auth: &AuthContext,
        notebook_id: Option<Uuid>,
        name: &str,
        permissions: &[String],
        rate_limit_rpm: i32,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<CreateApiKeyResponse, AppError>;

    async fn revoke_api_key(
        &self,
        auth: &AuthContext,
        notebook_id: Option<Uuid>,
        key_id: Uuid,
    ) -> Result<bool, AppError>;

    async fn list_notifications(
        &self,
        auth: &AuthContext,
        user_id: Uuid,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<NotificationRow>, AppError>;

    async fn mark_notification_read(
        &self,
        auth: &AuthContext,
        user_id: Uuid,
        notification_id: Uuid,
    ) -> Result<bool, AppError>;

    async fn list_orgs(
        &self,
        auth: &AuthContext,
        page: usize,
        per_page: usize,
    ) -> Result<Vec<AdminOrgInfo>, AppError>;

    async fn get_org(&self, auth: &AuthContext, org_id: OrgId) -> Result<AdminOrgInfo, AppError>;

    async fn list_users(
        &self,
        auth: &AuthContext,
        org_id: OrgId,
    ) -> Result<Vec<AdminUserInfo>, AppError>;

    async fn delete_user(
        &self,
        auth: &AuthContext,
        org_id: OrgId,
        user_id: Uuid,
    ) -> Result<(), AppError>;

    async fn get_usage(
        &self,
        auth: &AuthContext,
        org_id: OrgId,
        period: &str,
    ) -> Result<AdminUsageStats, AppError>;

    async fn set_org_blocked(
        &self,
        auth: &AuthContext,
        org_id: OrgId,
        blocked: bool,
    ) -> Result<(), AppError>;

    async fn list_audit_logs(
        &self,
        auth: &AuthContext,
        query: &AdminAuditLogQuery,
    ) -> Result<AdminAuditLogPage, AppError>;

    async fn export_audit_logs_csv(
        &self,
        auth: &AuthContext,
        query: &AdminAuditLogQuery,
    ) -> Result<String, AppError>;
}
