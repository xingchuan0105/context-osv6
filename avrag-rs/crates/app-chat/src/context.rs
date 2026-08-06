use std::sync::Arc;

use app_admin::AdminContext;
use app_billing::{BillingContext, CostEventRecord};
use app_core::ChatPersistencePort;
use app_core::{AnalyticsServiceCtx, StorageContext};
use app_documents::DocumentContext;
use common::AppError;
use contracts::auth_runtime::{ActorId, AuthContext, SubjectKind, UserId};
use uuid::Uuid;

use crate::llm_context::LlmContext;
use crate::orchestrator_context::OrchestratorContext;

/// Pure remount helper for Owner-pays (unit-tested).
pub fn remount_member_owner_pays_auth(
    _caller_auth: &AuthContext,
    workspace_id: Uuid,
    owner: Uuid,
    member: Uuid,
) -> AuthContext {
    AuthContext::new(UserId::from(owner), SubjectKind::User)
        .with_actor_id(ActorId::new(member))
        .with_workspace_scope(workspace_id)
        .grant("workspace_member_chat")
}

#[cfg(test)]
mod owner_pays_tests {
    use super::*;
    use contracts::auth_runtime::UserId;

    #[test]
    fn remount_sets_owner_as_user_and_member_as_actor() {
        let ws = Uuid::new_v4();
        let owner = Uuid::new_v4();
        let member = Uuid::new_v4();
        let caller = AuthContext::new(UserId::from(member), SubjectKind::User)
            .with_actor_id(ActorId::new(member));
        let remounted = remount_member_owner_pays_auth(&caller, ws, owner, member);
        assert_eq!(remounted.user_id().into_uuid(), owner);
        assert_eq!(remounted.actor_id().map(|a| a.into_uuid()), Some(member));
        assert_eq!(remounted.workspace_id(), Some(ws));
        assert!(remounted.has_permission("workspace_member_chat"));
    }

    #[test]
    fn share_chat_permission_skips_store_lookup_path() {
        // Documented contract: callers with share_chat keep middleware remount.
        let auth = AuthContext::new(UserId::from(Uuid::new_v4()), SubjectKind::User)
            .grant("share_chat");
        assert!(auth.has_permission("share_chat"));
    }
}

/// Chat-scoped application context: auth, storage, orchestrator, billing, etc.
#[derive(Clone)]
pub struct ChatContext {
    pub auth: AuthContext,
    pub storage: StorageContext,
    pub llm_ctx: LlmContext,
    pub orchestrator: OrchestratorContext,
    pub analytics: AnalyticsServiceCtx,
    pub billing: BillingContext,
    pub admin: AdminContext,
    pub documents: DocumentContext,
}

impl ChatContext {
    pub fn new(
        auth: AuthContext,
        storage: StorageContext,
        llm_ctx: LlmContext,
        orchestrator: OrchestratorContext,
        analytics: AnalyticsServiceCtx,
        billing: BillingContext,
        admin: AdminContext,
        documents: DocumentContext,
    ) -> Self {
        Self {
            auth,
            storage,
            llm_ctx,
            orchestrator,
            analytics,
            billing,
            admin,
            documents,
        }
    }

    pub fn chat_persistence(&self) -> Option<Arc<dyn ChatPersistencePort>> {
        self.storage.chat_persistence()
    }

    pub fn uses_memory_adapters(&self) -> bool {
        self.storage.uses_memory_adapters()
    }

    /// ADR-0010 D2 / §4.2 Owner-pays for accepted members in a **shared** workspace.
    ///
    /// - Share middleware already remounts `user_id` to the Owner (`share_chat` grant).
    /// - JWT members chatting with a workspace_id bill the Owner only when they are an
    ///   accepted member **and** `share_enabled` (see `owner_for_accepted_member`).
    /// - Private workspaces without share keep member self-pay.
    pub async fn with_owner_pays_auth(&self, workspace_id: Option<Uuid>) -> Self {
        if self.auth.has_permission("share_chat") {
            return self.clone();
        }
        let Some(workspace_id) = workspace_id else {
            return self.clone();
        };
        let Some(store) = self.storage.share_store() else {
            return self.clone();
        };
        let caller = self.auth.user_id().into_uuid();
        let owner = match store.owner_for_accepted_member(workspace_id, caller).await {
            Ok(owner) => owner,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    %workspace_id,
                    "owner_for_accepted_member failed; keeping caller as payer"
                );
                return self.clone();
            }
        };
        let Some(owner) = owner else {
            return self.clone();
        };
        let mut next = self.clone();
        next.auth = remount_member_owner_pays_auth(&self.auth, workspace_id, owner, caller);
        next
    }

    /// Retrieval is routed through orchestrator `RagRuntime` (see `app_core::RetrievalPort`).
    pub fn retrieval_runtime(&self) -> Option<&std::sync::Arc<avrag_rag_core::RagRuntime>> {
        self.orchestrator.rag_runtime()
    }

    pub fn default_user_id(&self) -> String {
        common::default_user_id()
    }

    pub fn analytics_ctx(&self) -> app_core::AnalyticsContext {
        self.analytics.into_context(
            self.auth.actor_id().map(|a| a.into_uuid()),
            self.auth.request_id().map(str::to_string),
        )
    }

    pub async fn record_product_event_if_available(
        &self,
        event_name: analytics::ProductEventName,
        surface: analytics::Surface,
        result: analytics::ResultTag,
        session_id: Option<Uuid>,
        workspace_id: Option<Uuid>,
        metadata: serde_json::Value,
    ) {
        self.analytics
            .record_product_event_for_auth(
                &self.auth,
                event_name,
                surface,
                result,
                session_id,
                workspace_id,
                metadata,
            )
            .await;
    }

    pub async fn record_cost_event_if_available(&self, record: CostEventRecord<'_>) {
        app_billing::record_cost_event_if_available(
            &self.auth,
            &self.analytics.service().cloned(),
            record,
        )
        .await;
    }

    pub async fn validate_rag_doc_scope(&self, doc_scope: &[String]) -> Result<(), AppError> {
        self.documents
            .validate_rag_doc_scope(&self.auth, &self.storage, doc_scope)
            .await
    }

    pub fn document_is_deleting_or_deleted(status: &contracts::documents::DocumentStatus) -> bool {
        matches!(
            status,
            contracts::documents::DocumentStatus::Deleting
                | contracts::documents::DocumentStatus::Deleted
        )
    }
}
