mod support;

use std::sync::Arc;

use app_core::{
    PublicShareChatContextSnapshot, ShareAccessLevel, ShareStorePort, SharedKnowledgeBaseSnapshot,
    SharedWorkspaceSnapshot, SharedShareInfoSnapshot, SharedSourceSnapshot,
};
use contracts::auth_runtime::{ActorId, AuthContext, UserId, SubjectKind};
use avrag_share::{AccessLevel, ShareService};
use support::MemoryShareStore;
use uuid::Uuid;

fn user_auth(user_id: Uuid) -> AuthContext {
    AuthContext::new(UserId::from(Uuid::new_v4()), SubjectKind::User)
        .with_actor_id(ActorId::new(user_id))
        .with_request_id("share-behavior-test")
}

#[tokio::test]
async fn load_shared_workspace_maps_snapshot_fields_to_payload() {
    let store = Arc::new(MemoryShareStore::new());
    let token = "public-read-token";
    store
        .seed_shared_workspace(
            token,
            SharedWorkspaceSnapshot {
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
                owner: Some(app_core::ShareOwnerCardSnapshot {
                    user_id: Some("u1".to_string()),
                    display_name: "Ada".to_string(),
                    bio: Some("Notes owner".to_string()),
                    contact_url: Some("https://example.test".to_string()),
                    avatar_url: Some("/api/public/users/u1/media/avatar".to_string()),
                    banner_url: None,
                    profile_enabled: true,
                }),
            },
        )
        .await;

    let service = ShareService::new(store);
    let payload = service
        .load_shared_workspace(token)
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
    let owner = payload.owner.expect("owner card");
    assert_eq!(owner.display_name, "Ada");
    assert_eq!(owner.bio.as_deref(), Some("Notes owner"));
    assert_eq!(owner.contact_url.as_deref(), Some("https://example.test"));
    assert_eq!(
        owner.avatar_url.as_deref(),
        Some("/api/public/users/u1/media/avatar")
    );
    assert!(owner.profile_enabled);
    assert_eq!(payload.sources[0].status, "ready");
}

#[tokio::test]
async fn load_shared_workspace_returns_none_for_unknown_token() {
    let service = ShareService::new(Arc::new(MemoryShareStore::new()));

    let payload = service
        .load_shared_workspace("missing-token")
        .await
        .expect("load should succeed");

    assert!(payload.is_none());
}

#[tokio::test]
async fn resolve_public_share_chat_context_maps_snapshot_to_domain() {
    let store = Arc::new(MemoryShareStore::new());
    let token = "chat-context-token";
    let owner_user_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    store
        .seed_public_chat_context(
            token,
            PublicShareChatContextSnapshot {
                owner_user_id,
                workspace_id,
                access_level: ShareAccessLevel::Read,
                workspace_visibility: "public".to_string(),
                share_enabled: true,
                anon_question_limit: 10,
                member_question_limit: None,
            },
        )
        .await;

    let service = ShareService::new(store);
    let context = service
        .resolve_public_share_chat_context(token)
        .await
        .expect("resolve should succeed")
        .expect("token should resolve to chat context");

    assert_eq!(context.owner_user_id, owner_user_id);
    assert_eq!(context.workspace_id, workspace_id);
    assert_eq!(context.access_level, AccessLevel::Read);
    assert!(context.allows_anonymous_chat());
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
    let workspace_id = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    store.seed_workspace_owner(workspace_id, owner_id).await;

    let service = ShareService::new(store.clone());
    let member = service
        .invite_member(
            &user_auth(owner_id),
            &workspace_id.to_string(),
            "collaborator@example.com",
            AccessLevel::Write,
        )
        .await
        .expect("owner should invite member");

    assert_eq!(member.workspace_id, workspace_id.to_string());
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
async fn owner_for_accepted_member_requires_share_enabled() {
    let store = Arc::new(MemoryShareStore::new());
    let workspace_id = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    let member_id = Uuid::new_v4();
    store.seed_workspace_owner(workspace_id, owner_id).await;
    store
        .seed_member_access(workspace_id, member_id, "viewer")
        .await;

    // Private collab (share off): member self-pay.
    let none = store
        .owner_for_accepted_member(workspace_id, member_id)
        .await
        .expect("lookup ok");
    assert_eq!(none, None);

    store.set_share_enabled(workspace_id, true).await;
    let owner = store
        .owner_for_accepted_member(workspace_id, member_id)
        .await
        .expect("lookup ok");
    assert_eq!(owner, Some(owner_id));

    // Non-member never remounts.
    let stranger = store
        .owner_for_accepted_member(workspace_id, Uuid::new_v4())
        .await
        .expect("lookup ok");
    assert_eq!(stranger, None);
}

#[tokio::test]
async fn invite_accept_url_includes_member_id() {
    // Contract for invite email links (handlers/workspaces/share.rs).
    let workspace_id = "ws-1111";
    let member_id = "mem-2222";
    let base = "https://app.example.test/";
    let accept_url = format!(
        "{}/invite/{workspace_id}/{member_id}",
        base.trim_end_matches('/')
    );
    assert_eq!(
        accept_url,
        "https://app.example.test/invite/ws-1111/mem-2222"
    );
    assert!(accept_url.contains(member_id));
    assert!(!accept_url.contains("share?invite="));
}

#[tokio::test]
async fn non_owner_invite_is_rejected_before_store() {
    let store = Arc::new(MemoryShareStore::new());
    let workspace_id = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    let viewer_id = Uuid::new_v4();
    store.seed_workspace_owner(workspace_id, owner_id).await;
    store
        .seed_member_access(workspace_id, viewer_id, "viewer")
        .await;

    let service = ShareService::new(store.clone());
    let error = service
        .invite_member(
            &user_auth(viewer_id),
            &workspace_id.to_string(),
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

#[tokio::test]
async fn list_public_shares_for_owner_returns_active_share_with_contract_fields() {
    let store = Arc::new(MemoryShareStore::new());
    let owner_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    store
        .seed_public_owner_workspace(workspace_id, owner_id, "Quarterly Review", 3)
        .await;
    let token = store
        .create_share_token(&user_auth(owner_id), workspace_id, ShareAccessLevel::Read, None)
        .await
        .expect("token minted");

    let service = ShareService::new(store);
    let items = service
        .list_public_shares_for_owner(owner_id)
        .await
        .expect("list should succeed");

    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(item.workspace_id, workspace_id.to_string());
    assert_eq!(item.title, "Quarterly Review");
    assert_eq!(item.share_token, token);
    assert_eq!(item.access_level, "partial");
    assert!(!item.allow_download);
    assert_eq!(item.source_count, 3);
}

#[tokio::test]
async fn list_public_shares_for_owner_excludes_revoked_and_expired_tokens() {
    let store = Arc::new(MemoryShareStore::new());
    let owner_id = Uuid::new_v4();
    let active_ws = Uuid::new_v4();
    let revoked_ws = Uuid::new_v4();
    let expired_ws = Uuid::new_v4();
    for (workspace_id, title) in [
        (active_ws, "Active"),
        (revoked_ws, "Revoked"),
        (expired_ws, "Expired"),
    ] {
        store
            .seed_public_owner_workspace(workspace_id, owner_id, title, 1)
            .await;
    }
    let auth = user_auth(owner_id);
    store
        .create_share_token(&auth, active_ws, ShareAccessLevel::Read, None)
        .await
        .expect("token minted");
    let revoked_token = store
        .create_share_token(&auth, revoked_ws, ShareAccessLevel::Read, None)
        .await
        .expect("token minted");
    store
        .revoke_token(&auth, &revoked_token)
        .await
        .expect("revoke should succeed");
    store
        .create_share_token(
            &auth,
            expired_ws,
            ShareAccessLevel::Read,
            Some(chrono::Utc::now() - chrono::Duration::hours(1)),
        )
        .await
        .expect("token minted");

    let service = ShareService::new(store);
    let items = service
        .list_public_shares_for_owner(owner_id)
        .await
        .expect("list should succeed");

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].workspace_id, active_ws.to_string());
}

#[tokio::test]
async fn list_public_shares_for_owner_dedupes_multiple_tokens_per_workspace() {
    let store = Arc::new(MemoryShareStore::new());
    let owner_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    store
        .seed_public_owner_workspace(workspace_id, owner_id, "Shared KB", 2)
        .await;
    let auth = user_auth(owner_id);
    store
        .create_share_token(&auth, workspace_id, ShareAccessLevel::Read, None)
        .await
        .expect("token minted");
    store
        .create_share_token(&auth, workspace_id, ShareAccessLevel::Write, None)
        .await
        .expect("token minted");

    let service = ShareService::new(store);
    let items = service
        .list_public_shares_for_owner(owner_id)
        .await
        .expect("list should succeed");

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].workspace_id, workspace_id.to_string());
}

#[tokio::test]
async fn list_public_shares_for_owner_excludes_other_owners() {
    let store = Arc::new(MemoryShareStore::new());
    let owner_id = Uuid::new_v4();
    let other_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    store
        .seed_public_owner_workspace(workspace_id, other_id, "Not Yours", 1)
        .await;
    store
        .create_share_token(&user_auth(other_id), workspace_id, ShareAccessLevel::Read, None)
        .await
        .expect("token minted");

    let service = ShareService::new(store);
    let items = service
        .list_public_shares_for_owner(owner_id)
        .await
        .expect("list should succeed");

    assert!(items.is_empty());
}

#[test]
fn share_owner_card_from_profile_maps_public_fields() {
    let user_id = Uuid::new_v4();
    let profile = app_core::AuthUserProfile {
        user_id,
        owner_user_id: user_id,
        email: "ada@example.test".to_string(),
        full_name: Some("  Ada Lovelace  ".to_string()),
        bio: Some("  Notes owner  ".to_string()),
        contact_url: None,
        avatar_object_path: Some("avatars/ada.png".to_string()),
        banner_object_path: Some("banners/ada.png".to_string()),
        public_profile_enabled: true,
    };

    let card = avrag_share::ShareOwnerCard::from_profile(&profile);

    let expected_avatar = format!("/api/public/users/{user_id}/media/avatar");
    let expected_banner = format!("/api/public/users/{user_id}/media/banner");
    assert_eq!(card.display_name, "Ada Lovelace");
    assert_eq!(card.bio.as_deref(), Some("Notes owner"));
    assert_eq!(card.contact_url, None);
    assert_eq!(card.avatar_url.as_deref(), Some(expected_avatar.as_str()));
    assert_eq!(card.banner_url.as_deref(), Some(expected_banner.as_str()));
    assert!(card.profile_enabled);
}

#[test]
fn share_owner_card_from_profile_falls_back_to_email_local_part() {
    let user_id = Uuid::new_v4();
    let profile = app_core::AuthUserProfile {
        user_id,
        owner_user_id: user_id,
        email: "ada@example.test".to_string(),
        full_name: Some("   ".to_string()),
        bio: None,
        contact_url: None,
        avatar_object_path: None,
        banner_object_path: None,
        public_profile_enabled: false,
    };

    let card = avrag_share::ShareOwnerCard::from_profile(&profile);

    assert_eq!(card.display_name, "ada");
    assert_eq!(card.avatar_url, None);
    assert!(!card.profile_enabled);
}
