use std::collections::HashSet;

use app_core::{admin_escape_ilike_pattern, AdminStorePort};
use async_trait::async_trait;
use avrag_auth::{ActorId, AuthContext, OrgId, SubjectKind};
use common::AppError;
use uuid::Uuid;

#[derive(Default)]
struct OrgBlockingStore {
    org_ids: HashSet<OrgId>,
}

#[async_trait]
impl AdminStorePort for OrgBlockingStore {
    async fn ensure_admin_access(&self, _auth: &AuthContext) -> Result<(), AppError> {
        Ok(())
    }

    async fn billing_overview(
        &self,
        _auth: &AuthContext,
    ) -> Result<app_core::AdminBillingOverview, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn rag_health(
        &self,
        _auth: &AuthContext,
    ) -> Result<app_core::AdminRagHealthStatus, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn worker_status(
        &self,
        _auth: &AuthContext,
    ) -> Result<app_core::AdminWorkerStatus, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn degradation_status(
        &self,
        _auth: &AuthContext,
    ) -> Result<app_core::AdminDegradationStatus, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn list_feature_flags(
        &self,
        _auth: &AuthContext,
    ) -> Result<Vec<app_core::AdminFeatureFlagEntry>, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn list_feature_flag_change_requests(
        &self,
        _auth: &AuthContext,
        _status: Option<&str>,
    ) -> Result<Vec<app_core::AdminFeatureFlagChangeRequest>, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn create_feature_flag_change_request(
        &self,
        _auth: &AuthContext,
        _key: &str,
        _enabled: bool,
        _reason: &str,
    ) -> Result<app_core::AdminFeatureFlagChangeRequest, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn review_feature_flag_change_request(
        &self,
        _auth: &AuthContext,
        _request_id: &str,
        _approved: bool,
        _review_note: Option<&str>,
    ) -> Result<app_core::AdminFeatureFlagChangeRequest, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn get_user_profile(
        &self,
        _auth: &AuthContext,
        _user_id: Uuid,
    ) -> Result<Option<app_core::UserProfileRow>, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn upsert_user_profile(
        &self,
        _auth: &AuthContext,
        _profile: &app_core::UserProfileRow,
    ) -> Result<(), AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn list_api_keys(
        &self,
        _auth: &AuthContext,
        _notebook_id: Option<Uuid>,
    ) -> Result<Vec<common::ApiKeyRow>, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn create_api_key(
        &self,
        _auth: &AuthContext,
        _notebook_id: Option<Uuid>,
        _name: &str,
        _permissions: &[String],
        _rate_limit_rpm: i32,
        _expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<common::CreateApiKeyResponse, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn revoke_api_key(
        &self,
        _auth: &AuthContext,
        _notebook_id: Option<Uuid>,
        _key_id: Uuid,
    ) -> Result<bool, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn list_notifications(
        &self,
        _auth: &AuthContext,
        _user_id: Uuid,
        _limit: usize,
        _offset: usize,
    ) -> Result<Vec<common::NotificationRow>, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn mark_notification_read(
        &self,
        _auth: &AuthContext,
        _user_id: Uuid,
        _notification_id: Uuid,
    ) -> Result<bool, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn list_orgs(
        &self,
        _auth: &AuthContext,
        _page: usize,
        _per_page: usize,
    ) -> Result<Vec<app_core::AdminOrgInfo>, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn get_org(
        &self,
        _auth: &AuthContext,
        org_id: OrgId,
    ) -> Result<app_core::AdminOrgInfo, AppError> {
        if self.org_ids.contains(&org_id) {
            Err(AppError::internal("not implemented"))
        } else {
            Err(AppError::not_found("org_not_found", "Organization not found"))
        }
    }

    async fn list_users(
        &self,
        _auth: &AuthContext,
        _org_id: OrgId,
    ) -> Result<Vec<app_core::AdminUserInfo>, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn delete_user(
        &self,
        _auth: &AuthContext,
        _org_id: OrgId,
        _user_id: Uuid,
    ) -> Result<(), AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn get_usage(
        &self,
        _auth: &AuthContext,
        _org_id: OrgId,
        _period: &str,
    ) -> Result<app_core::AdminUsageStats, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn set_org_blocked(
        &self,
        _auth: &AuthContext,
        org_id: OrgId,
        _blocked: bool,
    ) -> Result<(), AppError> {
        if self.org_ids.contains(&org_id) {
            Ok(())
        } else {
            Err(AppError::not_found(
                "org_not_found",
                "Organization not found",
            ))
        }
    }

    async fn list_audit_logs(
        &self,
        _auth: &AuthContext,
        _query: &app_core::AdminAuditLogQuery,
    ) -> Result<app_core::AdminAuditLogPage, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn export_audit_logs_csv(
        &self,
        _auth: &AuthContext,
        _query: &app_core::AdminAuditLogQuery,
    ) -> Result<String, AppError> {
        Err(AppError::internal("not implemented"))
    }
}

fn test_auth() -> AuthContext {
    AuthContext::new(OrgId::from(Uuid::nil()), SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::nil()))
        .with_request_id("admin-store-behavior")
}

#[tokio::test]
async fn set_org_blocked_returns_not_found_for_missing_org() {
    let store = OrgBlockingStore::default();
    let auth = test_auth();
    let missing_org = OrgId::from(Uuid::new_v4());

    let error = store
        .set_org_blocked(&auth, missing_org, true)
        .await
        .expect_err("blocking a missing org should fail");

    assert_eq!(error.code(), "org_not_found");
}

#[tokio::test]
async fn set_org_blocked_succeeds_for_existing_org() {
    let org_id = OrgId::from(Uuid::new_v4());
    let mut store = OrgBlockingStore::default();
    store.org_ids.insert(org_id);
    let auth = test_auth();

    store
        .set_org_blocked(&auth, org_id, true)
        .await
        .expect("blocking an existing org should succeed");
}

#[test]
fn audit_search_escape_preserves_literal_percent_and_underscore() {
    let escaped = admin_escape_ilike_pattern("100%_done");
    assert_eq!(escaped, r"100\%\_done");
    assert_eq!(format!("%{escaped}%"), r"%100\%\_done%");
}
