use std::collections::HashMap;
use std::sync::Arc;

use app_core::{
    NotebookAccessSnapshot, PublicShareChatContextSnapshot, ShareAccessLevel, ShareAccessLogEntry,
    ShareAnalyticsEntry, ShareNotebookMember, ShareStorePort, SharedNotebookSnapshot,
};
use async_trait::async_trait;
use avrag_auth::{ActorId, AuthContext, OrgId, SubjectKind};
use avrag_share::{AccessLevel, ShareService};
use chrono::{DateTime, Utc};
use common::AppError;
use tokio::sync::RwLock;
use uuid::Uuid;

#[test]
fn share_modules_do_not_call_storage_pg_escape_hatch() {
    let forbidden = concat!("storage.", "pg(");
    let sources = [
        include_str!("../src/access.rs"),
        include_str!("../src/handlers.rs"),
        include_str!("../src/members.rs"),
        include_str!("../src/public_read.rs"),
        include_str!("../src/sharing.rs"),
    ];
    for source in sources {
        assert!(
            !source.contains(forbidden),
            "avrag-share must use ShareStorePort, not the pg escape hatch"
        );
    }
}

#[derive(Clone)]
struct TokenRecord {
    notebook_id: Uuid,
    access_level: ShareAccessLevel,
    expires_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Default)]
struct MemoryShareStore {
    notebooks: Arc<RwLock<HashMap<Uuid, NotebookAccessSnapshot>>>,
    tokens: Arc<RwLock<HashMap<String, TokenRecord>>>,
}

impl MemoryShareStore {
    fn new() -> Self {
        Self::default()
    }

    async fn seed_notebook_owner(&self, notebook_id: Uuid, owner_id: Uuid) {
        self.notebooks.write().await.insert(
            notebook_id,
            NotebookAccessSnapshot {
                owner_id: Some(owner_id),
                notebook_access_level: "private".to_string(),
            },
        );
    }
}

#[async_trait]
impl ShareStorePort for MemoryShareStore {
    async fn query_notebook_access(
        &self,
        _auth: &AuthContext,
        notebook_id: Uuid,
    ) -> Result<Option<NotebookAccessSnapshot>, AppError> {
        Ok(self.notebooks.read().await.get(&notebook_id).cloned())
    }

    async fn query_member_access(
        &self,
        _auth: &AuthContext,
        _notebook_id: Uuid,
        _user_id: Uuid,
    ) -> Result<Option<String>, AppError> {
        Ok(None)
    }

    async fn get_share_settings(
        &self,
        _auth: &AuthContext,
        _notebook_id: Uuid,
    ) -> Result<(String, bool, Vec<app_core::ShareTokenSnapshot>), AppError> {
        Ok(("private".to_string(), false, Vec::new()))
    }

    async fn list_members(
        &self,
        _auth: &AuthContext,
        _notebook_id: Uuid,
    ) -> Result<Vec<ShareNotebookMember>, AppError> {
        Ok(Vec::new())
    }

    async fn update_notebook_access_level(
        &self,
        _auth: &AuthContext,
        _notebook_id: Uuid,
        _access_level: &str,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn update_share_settings(
        &self,
        _auth: &AuthContext,
        _notebook_id: Uuid,
        _access_level: Option<&str>,
        _allow_download: Option<bool>,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn create_share_token(
        &self,
        _auth: &AuthContext,
        notebook_id: Uuid,
        access_level: ShareAccessLevel,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<String, AppError> {
        let token = Uuid::new_v4().to_string();
        self.tokens.write().await.insert(
            token.clone(),
            TokenRecord {
                notebook_id,
                access_level,
                expires_at,
                revoked_at: None,
            },
        );
        Ok(token)
    }

    async fn validate_token(
        &self,
        token: &str,
    ) -> Result<Option<(Uuid, ShareAccessLevel)>, AppError> {
        let tokens = self.tokens.read().await;
        let Some(record) = tokens.get(token) else {
            return Ok(None);
        };
        if record.revoked_at.is_some() {
            return Ok(None);
        }
        if let Some(expires_at) = record.expires_at {
            if expires_at <= Utc::now() {
                return Ok(None);
            }
        }
        Ok(Some((record.notebook_id, record.access_level)))
    }

    async fn revoke_token(
        &self,
        _auth: &AuthContext,
        token: &str,
    ) -> Result<Option<Uuid>, AppError> {
        let mut tokens = self.tokens.write().await;
        if let Some(record) = tokens.get_mut(token) {
            record.revoked_at = Some(Utc::now());
            return Ok(Some(record.notebook_id));
        }
        Ok(None)
    }

    async fn get_share_analytics(
        &self,
        _auth: &AuthContext,
        _notebook_id: Uuid,
    ) -> Result<Vec<ShareAnalyticsEntry>, AppError> {
        Ok(Vec::new())
    }

    async fn get_share_access_logs(
        &self,
        _auth: &AuthContext,
        _notebook_id: Uuid,
        _limit: usize,
    ) -> Result<Vec<ShareAccessLogEntry>, AppError> {
        Ok(Vec::new())
    }

    async fn load_shared_notebook(
        &self,
        _token: &str,
    ) -> Result<Option<SharedNotebookSnapshot>, AppError> {
        Ok(None)
    }

    async fn resolve_public_share_chat_context(
        &self,
        _token: &str,
    ) -> Result<Option<PublicShareChatContextSnapshot>, AppError> {
        Ok(None)
    }

    async fn invite_member(
        &self,
        _auth: &AuthContext,
        _notebook_id: Uuid,
        _email: &str,
        _access_level: ShareAccessLevel,
    ) -> Result<ShareNotebookMember, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn accept_invite(
        &self,
        _auth: &AuthContext,
        _notebook_id: Uuid,
        _member_id: Uuid,
        _actor_id: Uuid,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn decline_invite(
        &self,
        _auth: &AuthContext,
        _notebook_id: Uuid,
        _member_id: Uuid,
        _actor_id: Uuid,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn add_member(
        &self,
        _auth: &AuthContext,
        _notebook_id: Uuid,
        _user_id: Uuid,
        _access_level: ShareAccessLevel,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn remove_member(
        &self,
        _auth: &AuthContext,
        _notebook_id: Uuid,
        _member_id: Uuid,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn record_share_product_event(
        &self,
        _event: analytics::ProductEvent,
    ) -> Result<(), AppError> {
        Ok(())
    }
}

fn owner_auth(owner_id: Uuid) -> AuthContext {
    AuthContext::new(OrgId::from(Uuid::new_v4()), SubjectKind::User)
        .with_actor_id(ActorId::new(owner_id))
        .with_request_id("share-port-contract")
}

#[tokio::test]
async fn create_share_token_round_trips_through_validate_token() {
    let store = Arc::new(MemoryShareStore::new());
    let notebook_id = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    store.seed_notebook_owner(notebook_id, owner_id).await;

    let service = ShareService::new(store);
    let auth = owner_auth(owner_id);
    let token = service
        .create_share_token(&auth, &notebook_id.to_string(), AccessLevel::Read, None)
        .await
        .expect("owner should create share token");

    let validated = service
        .validate_token(&token)
        .await
        .expect("validate should succeed")
        .expect("token should resolve");

    assert_eq!(validated.0, notebook_id.to_string());
    assert_eq!(validated.1, AccessLevel::Read);
}

#[tokio::test]
async fn validate_token_returns_none_for_unknown_token() {
    let service = ShareService::new(Arc::new(MemoryShareStore::new()));

    let validated = service
        .validate_token("missing-share-token")
        .await
        .expect("validate should succeed");

    assert!(validated.is_none());
}
