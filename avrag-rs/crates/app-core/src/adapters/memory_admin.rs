//! In-memory AdminStorePort — API keys, preferences, notifications for memory mode.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use common::{
    ApiKeyRow, AppError, CreateApiKeyResponse, NotificationRow, new_id, now_rfc3339,
};
use contracts::auth_runtime::{AuthContext, OrgId};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::admin_domain::{
    AdminAuditLogPage, AdminAuditLogQuery, AdminBillingOverview, AdminDegradationStatus,
    AdminFeatureFlagChangeRequest, AdminFeatureFlagEntry, AdminOrgInfo, AdminRagHealthStatus,
    AdminUsageStats, AdminUserInfo, AdminWorkerStatus,
};
use crate::admin_store::AdminStorePort;
use crate::api_key::{
    MemoryApiKeyRecord, deactivate_memory_api_key, register_memory_api_key,
};
use crate::domain_rows::UserProfileRow;
use crate::state_types::MemoryState;

const ORG_KEY_BUCKET: &str = "__org__";

#[derive(Clone)]
pub struct MemoryAdminStore {
    state: Arc<RwLock<MemoryState>>,
    api_keys: Arc<RwLock<BTreeMap<String, Vec<ApiKeyRow>>>>,
    api_key_hashes: Arc<RwLock<BTreeMap<String, MemoryApiKeyRecord>>>,
}

impl MemoryAdminStore {
    pub fn new(
        state: Arc<RwLock<MemoryState>>,
        api_keys: Arc<RwLock<BTreeMap<String, Vec<ApiKeyRow>>>>,
        api_key_hashes: Arc<RwLock<BTreeMap<String, MemoryApiKeyRecord>>>,
    ) -> Self {
        Self {
            state,
            api_keys,
            api_key_hashes,
        }
    }

    fn bucket_key(notebook_id: Option<Uuid>) -> String {
        notebook_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| ORG_KEY_BUCKET.to_string())
    }
}

#[async_trait]
impl AdminStorePort for MemoryAdminStore {
    async fn ensure_admin_access(&self, _auth: &AuthContext) -> Result<(), AppError> {
        Ok(())
    }

    async fn billing_overview(
        &self,
        _auth: &AuthContext,
    ) -> Result<AdminBillingOverview, AppError> {
        Ok(AdminBillingOverview {
            active_subscriptions: 0,
            past_due_subscriptions: 0,
            unpaid_subscriptions: 0,
            canceled_subscriptions: 0,
        })
    }

    async fn rag_health(&self, _auth: &AuthContext) -> Result<AdminRagHealthStatus, AppError> {
        Ok(AdminRagHealthStatus {
            failed_documents: 0,
            queued_tasks: 0,
            processing_tasks: 0,
            dead_letter_tasks: 0,
            recent_guard_events: 0,
        })
    }

    async fn worker_status(&self, _auth: &AuthContext) -> Result<AdminWorkerStatus, AppError> {
        Ok(AdminWorkerStatus {
            runtime_mode: "memory",
            queued_tasks: 0,
            processing_tasks: 0,
            dead_letter_tasks: 0,
            failed_documents: 0,
        })
    }

    async fn degradation_status(
        &self,
        _auth: &AuthContext,
    ) -> Result<AdminDegradationStatus, AppError> {
        Ok(AdminDegradationStatus {
            failed_documents: 0,
            recent_guard_events: 0,
            share_access_events: 0,
        })
    }

    async fn list_feature_flags(
        &self,
        _auth: &AuthContext,
    ) -> Result<Vec<AdminFeatureFlagEntry>, AppError> {
        Ok(Vec::new())
    }

    async fn list_feature_flag_change_requests(
        &self,
        _auth: &AuthContext,
        _status: Option<&str>,
    ) -> Result<Vec<AdminFeatureFlagChangeRequest>, AppError> {
        Ok(Vec::new())
    }

    async fn create_feature_flag_change_request(
        &self,
        _auth: &AuthContext,
        _key: &str,
        _enabled: bool,
        _reason: &str,
    ) -> Result<AdminFeatureFlagChangeRequest, AppError> {
        Err(AppError::internal(
            "feature flags are not available in memory mode",
        ))
    }

    async fn review_feature_flag_change_request(
        &self,
        _auth: &AuthContext,
        _request_id: &str,
        _approved: bool,
        _review_note: Option<&str>,
    ) -> Result<AdminFeatureFlagChangeRequest, AppError> {
        Err(AppError::internal(
            "feature flags are not available in memory mode",
        ))
    }

    async fn get_user_profile(
        &self,
        auth: &AuthContext,
        user_id: Uuid,
    ) -> Result<Option<UserProfileRow>, AppError> {
        let state = self.state.read().await;
        let Some(preferences) = state.user_preferences.get(&user_id.to_string()) else {
            return Ok(None);
        };
        Ok(Some(UserProfileRow {
            user_id,
            org_id: auth.org_id(),
            expertise_domains: Vec::new(),
            preferred_answer_style: None,
            frequently_asked_topics: Vec::new(),
            custom_preferences: serde_json::to_value(preferences)
                .unwrap_or_else(|_| serde_json::json!({})),
            structured_profile: serde_json::json!({}),
            inferred_at: Utc::now(),
            inference_version: "preferences-v1".to_string(),
        }))
    }

    async fn upsert_user_profile(
        &self,
        _auth: &AuthContext,
        profile: &UserProfileRow,
    ) -> Result<(), AppError> {
        let preferences = serde_json::from_value(profile.custom_preferences.clone())
            .unwrap_or_default();
        let mut state = self.state.write().await;
        state
            .user_preferences
            .insert(profile.user_id.to_string(), preferences);
        Ok(())
    }

    async fn list_api_keys(
        &self,
        _auth: &AuthContext,
        notebook_id: Option<Uuid>,
    ) -> Result<Vec<ApiKeyRow>, AppError> {
        let bucket = Self::bucket_key(notebook_id);
        let keys = self.api_keys.read().await;
        Ok(keys.get(&bucket).cloned().unwrap_or_default())
    }

    async fn create_api_key(
        &self,
        auth: &AuthContext,
        notebook_id: Option<Uuid>,
        name: &str,
        permissions: &[String],
        rate_limit_rpm: i32,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<CreateApiKeyResponse, AppError> {
        let plaintext_key = format!("sk_{}", new_id().replace('-', ""));
        let rate_limit_rpm_u32 = u32::try_from(rate_limit_rpm).unwrap_or(60);
        let org_id = auth.org_id().to_string();
        let row = ApiKeyRow {
            id: new_id(),
            org_id: org_id.clone(),
            notebook_id: notebook_id
                .map(|id| id.to_string())
                .unwrap_or_default(),
            key_prefix: plaintext_key.chars().take(12).collect(),
            name: name.to_string(),
            permissions: permissions.to_vec(),
            rate_limit_rpm: rate_limit_rpm_u32,
            expires_at: expires_at.map(|value| value.to_rfc3339()),
            last_used_at: None,
            is_active: true,
            created_by: auth
                .actor_id()
                .map(|id| id.into_uuid().to_string())
                .unwrap_or_else(common::default_user_id),
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
        };
        let bucket = Self::bucket_key(notebook_id);
        {
            let mut keys = self.api_keys.write().await;
            keys.entry(bucket).or_default().push(row.clone());
        }
        register_memory_api_key(
            &self.api_key_hashes,
            &plaintext_key,
            MemoryApiKeyRecord {
                id: Uuid::parse_str(&row.id).unwrap_or_else(|_| Uuid::new_v4()),
                org_id: auth.org_id(),
                notebook_id,
                permissions: permissions.to_vec(),
                rate_limit_rpm: rate_limit_rpm_u32,
                is_active: true,
                expires_at,
            },
        )
        .await;
        Ok(CreateApiKeyResponse {
            api_key: row,
            plaintext_key,
        })
    }

    async fn revoke_api_key(
        &self,
        _auth: &AuthContext,
        notebook_id: Option<Uuid>,
        key_id: Uuid,
    ) -> Result<bool, AppError> {
        let bucket = Self::bucket_key(notebook_id);
        let key_id_str = key_id.to_string();
        let mut keys_map = self.api_keys.write().await;
        let keys = keys_map.entry(bucket).or_default();
        let before = keys.len();
        keys.retain(|item| item.id != key_id_str);
        if before == keys.len() {
            return Ok(false);
        }
        deactivate_memory_api_key(&self.api_key_hashes, &key_id_str).await;
        Ok(true)
    }

    async fn list_notifications(
        &self,
        auth: &AuthContext,
        user_id: Uuid,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<NotificationRow>, AppError> {
        let state = self.state.read().await;
        if state.notifications.is_empty() {
            return Ok(vec![NotificationRow {
                id: "notif-m1-skeleton".to_string(),
                org_id: auth.org_id().to_string(),
                user_id: user_id.to_string(),
                event_type: "system.degrade".to_string(),
                title: "M1/M2 skeleton running".to_string(),
                body: "Rust API is serving placeholder notebook/document/chat flows with explicit degrade trace.".to_string(),
                data: BTreeMap::new(),
                read_at: None,
                created_at: now_rfc3339(),
                updated_at: now_rfc3339(),
            }]);
        }
        let items: Vec<_> = state
            .notifications
            .iter()
            .filter(|n| n.user_id == user_id.to_string())
            .cloned()
            .collect();
        Ok(items.into_iter().skip(offset).take(limit).collect())
    }

    async fn mark_notification_read(
        &self,
        _auth: &AuthContext,
        _user_id: Uuid,
        notification_id: Uuid,
    ) -> Result<bool, AppError> {
        let id = notification_id.to_string();
        let mut state = self.state.write().await;
        if let Some(item) = state.notifications.iter_mut().find(|item| item.id == id) {
            if item.read_at.is_none() {
                item.read_at = Some(now_rfc3339());
                item.updated_at = now_rfc3339();
            }
            return Ok(true);
        }
        Ok(false)
    }

    async fn list_orgs(
        &self,
        _auth: &AuthContext,
        _page: usize,
        _per_page: usize,
    ) -> Result<Vec<AdminOrgInfo>, AppError> {
        Ok(Vec::new())
    }

    async fn get_org(&self, _auth: &AuthContext, _org_id: OrgId) -> Result<AdminOrgInfo, AppError> {
        Err(AppError::not_found(
            "org_not_found",
            "Organization not found",
        ))
    }

    async fn list_users(
        &self,
        _auth: &AuthContext,
        _org_id: OrgId,
    ) -> Result<Vec<AdminUserInfo>, AppError> {
        Ok(Vec::new())
    }

    async fn delete_user(
        &self,
        _auth: &AuthContext,
        _org_id: OrgId,
        _user_id: Uuid,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn get_usage(
        &self,
        _auth: &AuthContext,
        _org_id: OrgId,
        _period: &str,
    ) -> Result<AdminUsageStats, AppError> {
        Err(AppError::not_found(
            "org_not_found",
            "Organization not found",
        ))
    }

    async fn set_org_blocked(
        &self,
        _auth: &AuthContext,
        _org_id: OrgId,
        _blocked: bool,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn list_audit_logs(
        &self,
        _auth: &AuthContext,
        _query: &AdminAuditLogQuery,
    ) -> Result<AdminAuditLogPage, AppError> {
        Ok(AdminAuditLogPage {
            items: Vec::new(),
            total: 0,
            page: 1,
            per_page: 50,
        })
    }

    async fn export_audit_logs_csv(
        &self,
        _auth: &AuthContext,
        _query: &AdminAuditLogQuery,
    ) -> Result<String, AppError> {
        Ok(String::new())
    }
}
