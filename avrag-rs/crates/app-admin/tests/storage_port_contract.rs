use std::collections::BTreeMap;
use std::sync::Arc;

use app_admin::AdminContext;
use app_core::{AdminStorePort, MemoryState, ObjectStorePort, StorageContext};
use async_trait::async_trait;
use avrag_auth::{ActorId, AuthContext, OrgId, SubjectKind};
use common::{AppError, ApiKeyRow, CreateApiKeyRequest, NotificationRow};
use contracts::UserPreferences;
use tokio::sync::RwLock;
use uuid::Uuid;

struct TestObjectStore;

#[async_trait]
impl ObjectStorePort for TestObjectStore {
    async fn put(&self, _path: &str, _bytes: &[u8]) -> Result<(), AppError> {
        Ok(())
    }

    async fn put_stream(
        &self,
        _path: &str,
        _stream: app_core::ObjectStoreUploadStream,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn get(&self, _path: &str) -> Result<Vec<u8>, AppError> {
        Ok(Vec::new())
    }

    async fn head(
        &self,
        _path: &str,
    ) -> Result<app_core::ObjectStoreMetadata, app_core::ObjectStoreHeadError> {
        Err(app_core::ObjectStoreHeadError::NotFound {
            path: String::new(),
        })
    }

    async fn presigned_get_url(&self, _path: &str, _ttl_secs: u64) -> Result<String, AppError> {
        Ok(String::new())
    }
}

#[test]
fn admin_modules_do_not_call_storage_pg_escape_hatch() {
    let forbidden = concat!("storage.", "pg(");
    let sources = [
        include_str!("../src/admin_context.rs"),
        include_str!("../src/preferences.rs"),
    ];
    for source in sources {
        assert!(
            !source.contains(forbidden),
            "app-admin must use typed storage ports, not the pg escape hatch"
        );
    }
}

#[derive(Clone, Default)]
struct RecordingAdminStore {
    preference_reads: Arc<std::sync::atomic::AtomicUsize>,
}

#[async_trait]
impl AdminStorePort for RecordingAdminStore {
    async fn get_user_profile(
        &self,
        _auth: &AuthContext,
        _user_id: Uuid,
    ) -> Result<Option<app_core::UserProfileRow>, AppError> {
        self.preference_reads
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(None)
    }

    async fn upsert_user_profile(
        &self,
        _auth: &AuthContext,
        _profile: &app_core::UserProfileRow,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn list_api_keys(
        &self,
        _auth: &AuthContext,
        _notebook_id: Option<Uuid>,
    ) -> Result<Vec<ApiKeyRow>, AppError> {
        Ok(Vec::new())
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
        Ok(false)
    }

    async fn list_notifications(
        &self,
        _auth: &AuthContext,
        _user_id: Uuid,
        _limit: usize,
        _offset: usize,
    ) -> Result<Vec<NotificationRow>, AppError> {
        Ok(Vec::new())
    }

    async fn mark_notification_read(
        &self,
        _auth: &AuthContext,
        _user_id: Uuid,
        _notification_id: Uuid,
    ) -> Result<bool, AppError> {
        Ok(false)
    }

    async fn ensure_admin_access(&self, _auth: &AuthContext) -> Result<(), AppError> {
        Ok(())
    }

    async fn billing_overview(
        &self,
        _auth: &AuthContext,
    ) -> Result<app_core::AdminBillingOverview, AppError> {
        Ok(app_core::AdminBillingOverview {
            active_subscriptions: 0,
            past_due_subscriptions: 0,
            unpaid_subscriptions: 0,
            canceled_subscriptions: 0,
        })
    }

    async fn rag_health(
        &self,
        _auth: &AuthContext,
    ) -> Result<app_core::AdminRagHealthStatus, AppError> {
        Ok(app_core::AdminRagHealthStatus {
            failed_documents: 0,
            queued_tasks: 0,
            processing_tasks: 0,
            dead_letter_tasks: 0,
            recent_guard_events: 0,
        })
    }

    async fn worker_status(
        &self,
        _auth: &AuthContext,
    ) -> Result<app_core::AdminWorkerStatus, AppError> {
        Ok(app_core::AdminWorkerStatus {
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
    ) -> Result<app_core::AdminDegradationStatus, AppError> {
        Ok(app_core::AdminDegradationStatus {
            failed_documents: 0,
            recent_guard_events: 0,
            share_access_events: 0,
        })
    }

    async fn list_feature_flags(
        &self,
        _auth: &AuthContext,
    ) -> Result<Vec<app_core::AdminFeatureFlagEntry>, AppError> {
        Ok(Vec::new())
    }

    async fn list_feature_flag_change_requests(
        &self,
        _auth: &AuthContext,
        _status: Option<&str>,
    ) -> Result<Vec<app_core::AdminFeatureFlagChangeRequest>, AppError> {
        Ok(Vec::new())
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
}

fn test_auth() -> AuthContext {
    AuthContext::new(OrgId::from(Uuid::nil()), SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::nil()))
        .with_request_id("admin-port-contract")
}

fn memory_storage() -> StorageContext {
    StorageContext::new(
        None,
        false,
        None,
        None,
        None,
        None,
        None,
        Arc::new(RwLock::new(MemoryState::default())),
        Arc::new(RwLock::new(BTreeMap::new())),
        10 * 1024 * 1024,
        true,
        Arc::new(TestObjectStore),
        "http://localhost".to_string(),
        "/tmp/avrag-admin-test".to_string(),
        3600,
        3600,
    )
}

fn storage_with_admin_store(store: Arc<dyn AdminStorePort>) -> StorageContext {
    StorageContext::new(
        None,
        false,
        None,
        None,
        Some(store),
        None,
        None,
        Arc::new(RwLock::new(MemoryState::default())),
        Arc::new(RwLock::new(BTreeMap::new())),
        10 * 1024 * 1024,
        false,
        Arc::new(TestObjectStore),
        "http://localhost".to_string(),
        "/tmp/avrag-admin-test".to_string(),
        3600,
        3600,
    )
}

#[tokio::test]
async fn memory_mode_preferences_round_trip_without_ports() {
    let admin = AdminContext::new();
    let storage = memory_storage();
    let auth = test_auth();
    let user_id = auth.actor_id().unwrap().into_uuid();

    let mut prefs = UserPreferences::default();
    prefs
        .dashboard
        .favorite_notebook_ids
        .push("nb-contract-test".to_string());

    admin
        .save_user_preferences(&auth, &storage, user_id, &prefs)
        .await
        .unwrap();
    let loaded = admin
        .load_user_preferences(&auth, &storage, user_id)
        .await
        .unwrap();
    assert_eq!(
        loaded.dashboard.favorite_notebook_ids,
        vec!["nb-contract-test".to_string()]
    );
}

#[tokio::test]
async fn admin_store_port_is_used_when_wired() {
    let recorder = Arc::new(RecordingAdminStore::default());
    let reads = recorder.preference_reads.clone();
    let storage = storage_with_admin_store(recorder);
    let admin = AdminContext::new();
    let auth = test_auth();
    let user_id = auth.actor_id().unwrap().into_uuid();

    let prefs = admin
        .load_user_preferences(&auth, &storage, user_id)
        .await
        .unwrap();
    assert!(prefs.dashboard.favorite_notebook_ids.is_empty());
    assert_eq!(
        reads.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "load_user_preferences should delegate to AdminStorePort"
    );
}

#[tokio::test]
async fn memory_mode_api_keys_use_in_memory_map() {
    let admin = AdminContext::new();
    let storage = memory_storage();
    let auth = test_auth();
    let notebook_id = Uuid::new_v4().to_string();

    let created = admin
        .create_api_key(
            &auth,
            &storage,
            &notebook_id,
            CreateApiKeyRequest {
                name: "test-key".to_string(),
                permissions: vec!["query".to_string()],
                rate_limit_rpm: Some(30),
                expires_at: None,
            },
        )
        .await
        .unwrap();

    let listed = admin
        .list_api_keys(&auth, &storage, &notebook_id)
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.api_key.id);
}
