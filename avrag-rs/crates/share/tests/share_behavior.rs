mod support;

use std::sync::Arc;

use app_core::{
    PublicShareChatContextSnapshot, ShareAccessLevel, SharedKnowledgeBaseSnapshot,
    SharedNotebookSnapshot, SharedShareInfoSnapshot, SharedSourceSnapshot,
};
use contracts::auth_runtime::{ActorId, AuthContext, OrgId, SubjectKind};
use avrag_share::{AccessLevel, ShareService};
use support::MemoryShareStore;
use uuid::Uuid;

fn user_auth(user_id: Uuid) -> AuthContext {
    AuthContext::new(OrgId::from(Uuid::new_v4()), SubjectKind::User)
        .with_actor_id(ActorId::new(user_id))
        .with_request_id("share-behavior-test")
}

#[tokio::test]
async fn load_shared_notebook_maps_snapshot_fields_to_payload() {
    let store = Arc::new(MemoryShareStore::new());
    let token = "public-read-token";
    store
        .seed_shared_notebook(
            token,
            SharedNotebookSnapshot {
                knowledge_base: SharedKnowledgeBaseSnapshot {
                    id: "nb-1".to_string(),
                    title: "Quarterly Review".to_string(),
                    description: Some("Q1 notes".to_string()),
                },
                share: SharedShareInfoSnapshot {
                    permission: "partial".to_string(),
                    expires_at: Some("2030-01-01T00:00:00Z".to_string()),
                    allow_download: true,
                    scope: "sources".to_string(),
                },
                sources: vec![SharedSourceSnapshot {
                    id: "src-1".to_string(),
                    file_name: "report.pdf".to_string(),
                    status: "ready".to_string(),
                }],
            },
        )
        .await;

    let service = ShareService::new(store);
    let payload = service
        .load_shared_notebook(token)
        .await
        .expect("load should succeed")
        .expect("token should resolve to payload");

    assert_eq!(payload.knowledge_base.id, "nb-1");
    assert_eq!(payload.knowledge_base.title, "Quarterly Review");
    assert_eq!(
        payload.knowledge_base.description.as_deref(),
        Some("Q1 notes")
    );
    assert_eq!(payload.share.permission, "partial");
    assert_eq!(
        payload.share.expires_at.as_deref(),
        Some("2030-01-01T00:00:00Z")
    );
    assert!(payload.share.allow_download);
    assert_eq!(payload.share.scope, "sources");
    assert_eq!(payload.sources.len(), 1);
    assert_eq!(payload.sources[0].file_name, "report.pdf");
    assert_eq!(payload.sources[0].status, "ready");
}

#[tokio::test]
async fn load_shared_notebook_returns_none_for_unknown_token() {
    let service = ShareService::new(Arc::new(MemoryShareStore::new()));

    let payload = service
        .load_shared_notebook("missing-token")
        .await
        .expect("load should succeed");

    assert!(payload.is_none());
}

#[tokio::test]
async fn resolve_public_share_chat_context_maps_snapshot_to_domain() {
    let store = Arc::new(MemoryShareStore::new());
    let token = "chat-context-token";
    let org_id = Uuid::new_v4();
    let notebook_id = Uuid::new_v4();
    let owner_user_id = Uuid::new_v4();
    store
        .seed_public_chat_context(
            token,
            PublicShareChatContextSnapshot {
                org_id,
                notebook_id,
                owner_user_id,
                access_level: ShareAccessLevel::Read,
            },
        )
        .await;

    let service = ShareService::new(store);
    let context = service
        .resolve_public_share_chat_context(token)
        .await
        .expect("resolve should succeed")
        .expect("token should resolve to chat context");

    assert_eq!(context.org_id, org_id);
    assert_eq!(context.notebook_id, notebook_id);
    assert_eq!(context.owner_user_id, owner_user_id);
    assert_eq!(context.access_level, AccessLevel::Read);
}

#[tokio::test]
async fn resolve_public_share_chat_context_returns_none_for_unknown_token() {
    let service = ShareService::new(Arc::new(MemoryShareStore::new()));

    let context = service
        .resolve_public_share_chat_context("missing-token")
        .await
        .expect("resolve should succeed");

    assert!(context.is_none());
}

#[tokio::test]
async fn owner_can_invite_member() {
    let store = Arc::new(MemoryShareStore::new());
    let notebook_id = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    store.seed_notebook_owner(notebook_id, owner_id).await;

    let service = ShareService::new(store.clone());
    let member = service
        .invite_member(
            &user_auth(owner_id),
            &notebook_id.to_string(),
            "collaborator@example.com",
            AccessLevel::Write,
        )
        .await
        .expect("owner should invite member");

    assert_eq!(member.notebook_id, notebook_id.to_string());
    assert_eq!(member.email.as_deref(), Some("collaborator@example.com"));
    assert_eq!(member.access_level, AccessLevel::Write);
    assert_eq!(member.invite_status, "pending");
    assert_eq!(
        member.invited_by.as_deref(),
        Some(owner_id.to_string().as_str())
    );

    let stored = store.invited_members().await;
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].email.as_deref(), Some("collaborator@example.com"));
}

#[tokio::test]
async fn non_owner_invite_is_rejected_before_store() {
    let store = Arc::new(MemoryShareStore::new());
    let notebook_id = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    let viewer_id = Uuid::new_v4();
    store.seed_notebook_owner(notebook_id, owner_id).await;
    store
        .seed_member_access(notebook_id, viewer_id, "viewer")
        .await;

    let service = ShareService::new(store.clone());
    let error = service
        .invite_member(
            &user_auth(viewer_id),
            &notebook_id.to_string(),
            "blocked@example.com",
            AccessLevel::Read,
        )
        .await
        .expect_err("viewer should not invite members");

    assert!(
        error
            .to_string()
            .contains("insufficient permission to invite members"),
        "unexpected error: {error}"
    );
    assert!(
        store.invited_members().await.is_empty(),
        "store invite_member should not run for unauthorized callers"
    );
}
