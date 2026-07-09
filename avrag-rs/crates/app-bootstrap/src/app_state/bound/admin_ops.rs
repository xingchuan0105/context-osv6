//! Bound face — super-admin / ops console (AdminStorePort).
//!
//! Product handlers should use `state.admin_ops()`, not `state.admin_store()`.

use app_core::{
    AdminAuditLogPage, AdminAuditLogQuery, AdminBillingOverview, AdminDegradationStatus,
    AdminFeatureFlagChangeRequest, AdminFeatureFlagEntry, AdminOrgInfo, AdminRagHealthStatus,
    AdminStorePort, AdminUsageStats, AdminUserInfo, AdminWorkerStatus,
};
use common::AppError;
use contracts::auth_runtime::{AuthContext, OrgId};
use std::sync::Arc;
use uuid::Uuid;

pub struct BoundAdminOps<'a> {
    pub(crate) auth: &'a AuthContext,
    pub(crate) store: Option<Arc<dyn AdminStorePort>>,
}

impl<'a> BoundAdminOps<'a> {
    fn require_store(&self) -> Result<Arc<dyn AdminStorePort>, AppError> {
        self.store.clone().ok_or_else(|| {
            AppError::validation(
                "postgres_not_configured",
                "postgres backend is not configured",
            )
        })
    }

    fn require_actor(&self) -> Result<(), AppError> {
        if self.auth.actor_id().is_none() {
            return Err(AppError::unauthorized(
                "admin action requires an authenticated user",
            ));
        }
        Ok(())
    }

    pub async fn list_orgs(
        &self,
        page: usize,
        per_page: usize,
    ) -> Result<Vec<AdminOrgInfo>, AppError> {
        self.require_actor()?;
        let store = self.require_store()?;
        store.list_orgs(self.auth, page, per_page).await
    }

    pub async fn get_org(&self, org_id: OrgId) -> Result<AdminOrgInfo, AppError> {
        self.require_actor()?;
        let store = self.require_store()?;
        store.get_org(self.auth, org_id).await
    }

    pub async fn list_users(&self, org_id: OrgId) -> Result<Vec<AdminUserInfo>, AppError> {
        self.require_actor()?;
        let store = self.require_store()?;
        store.list_users(self.auth, org_id).await
    }

    pub async fn delete_user(&self, org_id: OrgId, user_id: Uuid) -> Result<(), AppError> {
        self.require_actor()?;
        let store = self.require_store()?;
        store.delete_user(self.auth, org_id, user_id).await
    }

    pub async fn get_usage(
        &self,
        org_id: OrgId,
        period: &str,
    ) -> Result<AdminUsageStats, AppError> {
        self.require_actor()?;
        let store = self.require_store()?;
        store.get_usage(self.auth, org_id, period).await
    }

    pub async fn set_org_blocked(&self, org_id: OrgId, blocked: bool) -> Result<(), AppError> {
        self.require_actor()?;
        let store = self.require_store()?;
        store.set_org_blocked(self.auth, org_id, blocked).await
    }

    pub async fn billing_overview(&self) -> Result<AdminBillingOverview, AppError> {
        self.require_actor()?;
        let store = self.require_store()?;
        store.billing_overview(self.auth).await
    }

    pub async fn rag_health(&self) -> Result<AdminRagHealthStatus, AppError> {
        self.require_actor()?;
        let store = self.require_store()?;
        store.rag_health(self.auth).await
    }

    pub async fn worker_status(&self) -> Result<AdminWorkerStatus, AppError> {
        self.require_actor()?;
        let store = self.require_store()?;
        store.worker_status(self.auth).await
    }

    pub async fn degradation_status(&self) -> Result<AdminDegradationStatus, AppError> {
        self.require_actor()?;
        let store = self.require_store()?;
        store.degradation_status(self.auth).await
    }

    pub async fn list_feature_flags(&self) -> Result<Vec<AdminFeatureFlagEntry>, AppError> {
        self.require_actor()?;
        let store = self.require_store()?;
        store.list_feature_flags(self.auth).await
    }

    pub async fn list_feature_flag_change_requests(
        &self,
        status: Option<&str>,
    ) -> Result<Vec<AdminFeatureFlagChangeRequest>, AppError> {
        self.require_actor()?;
        let store = self.require_store()?;
        store
            .list_feature_flag_change_requests(self.auth, status)
            .await
    }

    pub async fn create_feature_flag_change_request(
        &self,
        key: &str,
        enabled: bool,
        reason: &str,
    ) -> Result<AdminFeatureFlagChangeRequest, AppError> {
        self.require_actor()?;
        let store = self.require_store()?;
        store
            .create_feature_flag_change_request(self.auth, key, enabled, reason)
            .await
    }

    pub async fn review_feature_flag_change_request(
        &self,
        request_id: &str,
        approved: bool,
        review_note: Option<&str>,
    ) -> Result<AdminFeatureFlagChangeRequest, AppError> {
        self.require_actor()?;
        let store = self.require_store()?;
        store
            .review_feature_flag_change_request(self.auth, request_id, approved, review_note)
            .await
    }

    pub async fn list_audit_logs(
        &self,
        query: &AdminAuditLogQuery,
    ) -> Result<AdminAuditLogPage, AppError> {
        self.require_actor()?;
        let store = self.require_store()?;
        store.list_audit_logs(self.auth, query).await
    }

    pub async fn export_audit_logs_csv(
        &self,
        query: &AdminAuditLogQuery,
    ) -> Result<String, AppError> {
        self.require_actor()?;
        let store = self.require_store()?;
        store.export_audit_logs_csv(self.auth, query).await
    }
}
