mod support;

use std::sync::Arc;

use app_core::ShareStorePort;
use avrag_share::{
    max_shared_workspaces_for_plan, AccessLevel, ShareService, SHARE_WORKSPACE_QUOTA_EXCEEDED,
};
use common::AppError;
use contracts::auth_runtime::{ActorId, AuthContext, SubjectKind, UserId};
use support::MemoryShareStore;
use uuid::Uuid;

fn owner_auth(owner_id: Uuid) -> AuthContext {
    AuthContext::new(UserId::from(owner_id), SubjectKind::User)
        .with_actor_id(ActorId::new(owner_id))
        .with_request_id("share-quota-test")
}

#[test]
fn plan_policy_lookup_free_plus_pro() {
    assert_eq!(max_shared_workspaces_for_plan("free"), 3);
    assert_eq!(max_shared_workspaces_for_plan("plus"), 10);
    assert_eq!(max_shared_workspaces_for_plan("pro"), 100);
}

#[tokio::test]
async fn free_user_can_enable_three_shares_fourth_fails() {
    let store = Arc::new(MemoryShareStore::new());
    let owner_id = Uuid::new_v4();
    store.seed_owner_plan(owner_id, "free").await;
    let service = ShareService::new(store.clone());
    let auth = owner_auth(owner_id);

    for _ in 0..3 {
        let workspace_id = Uuid::new_v4();
        store.seed_workspace_owner(workspace_id, owner_id).await;
        service
            .update_access_level(&auth, &workspace_id.to_string(), "link")
            .await
            .expect("first three enables should succeed");
        assert!(store.is_share_enabled(workspace_id).await);
    }
    assert_eq!(store.count_enabled_for_owner(owner_id).await, 3);

    let fourth = Uuid::new_v4();
    store.seed_workspace_owner(fourth, owner_id).await;
    let err = service
        .update_access_level(&auth, &fourth.to_string(), "public")
        .await
        .expect_err("fourth enable must fail for free plan");
    let app = err
        .downcast_ref::<AppError>()
        .expect("quota error must be AppError");
    assert_eq!(app.code(), SHARE_WORKSPACE_QUOTA_EXCEEDED);
    assert_eq!(app.http_status(), 403);
    assert!(!store.is_share_enabled(fourth).await);
    assert_eq!(store.count_enabled_for_owner(owner_id).await, 3);

    // create_share_token on a non-enabled workspace also hits the gate
    let via_token = Uuid::new_v4();
    store.seed_workspace_owner(via_token, owner_id).await;
    let token_err = service
        .create_share_token(&auth, &via_token.to_string(), AccessLevel::Read, None)
        .await
        .expect_err("token create at quota must fail");
    assert_eq!(
        token_err
            .downcast_ref::<AppError>()
            .expect("AppError")
            .code(),
        SHARE_WORKSPACE_QUOTA_EXCEEDED
    );
}

#[tokio::test]
async fn disabling_share_frees_slot() {
    let store = Arc::new(MemoryShareStore::new());
    let owner_id = Uuid::new_v4();
    store.seed_owner_plan(owner_id, "free").await;
    let service = ShareService::new(store.clone());
    let auth = owner_auth(owner_id);

    let mut ids = Vec::new();
    for _ in 0..3 {
        let id = Uuid::new_v4();
        store.seed_workspace_owner(id, owner_id).await;
        service
            .update_access_level(&auth, &id.to_string(), "link")
            .await
            .unwrap();
        ids.push(id);
    }

    service
        .update_access_level(&auth, &ids[0].to_string(), "private")
        .await
        .expect("disable should succeed");
    assert!(!store.is_share_enabled(ids[0]).await);
    assert_eq!(store.count_enabled_for_owner(owner_id).await, 2);

    let again = Uuid::new_v4();
    store.seed_workspace_owner(again, owner_id).await;
    service
        .update_share_settings(&auth, &again.to_string(), Some("link"), None)
        .await
        .expect("can enable again after free slot");
    assert!(store.is_share_enabled(again).await);
    assert_eq!(store.count_enabled_for_owner(owner_id).await, 3);
}

#[tokio::test]
async fn plus_plan_allows_ten_enables() {
    let store = Arc::new(MemoryShareStore::new());
    let owner_id = Uuid::new_v4();
    store.seed_owner_plan(owner_id, "plus").await;
    let service = ShareService::new(store.clone());
    let auth = owner_auth(owner_id);

    for _ in 0..10 {
        let id = Uuid::new_v4();
        store.seed_workspace_owner(id, owner_id).await;
        service
            .create_share_token(&auth, &id.to_string(), AccessLevel::Read, None)
            .await
            .expect("plus allows 10 share-enabled workspaces");
    }
    assert_eq!(store.count_enabled_for_owner(owner_id).await, 10);

    let eleventh = Uuid::new_v4();
    store.seed_workspace_owner(eleventh, owner_id).await;
    let err = service
        .create_share_token(&auth, &eleventh.to_string(), AccessLevel::Read, None)
        .await
        .expect_err("11th must fail on plus");
    assert_eq!(
        err.downcast_ref::<AppError>().unwrap().code(),
        SHARE_WORKSPACE_QUOTA_EXCEEDED
    );
}

#[tokio::test]
async fn pro_plan_limit_is_one_hundred() {
    let store = Arc::new(MemoryShareStore::new());
    let owner_id = Uuid::new_v4();
    store.seed_owner_plan(owner_id, "pro").await;
    assert_eq!(
        store
            .max_shared_workspaces_for_owner(&owner_auth(owner_id), owner_id)
            .await
            .unwrap(),
        100
    );

    let service = ShareService::new(store.clone());
    let auth = owner_auth(owner_id);
    for _ in 0..100 {
        let id = Uuid::new_v4();
        store.seed_workspace_owner(id, owner_id).await;
        service
            .update_access_level(&auth, &id.to_string(), "link")
            .await
            .expect("pro allows 100");
    }
    let over = Uuid::new_v4();
    store.seed_workspace_owner(over, owner_id).await;
    let err = service
        .update_access_level(&auth, &over.to_string(), "link")
        .await
        .expect_err("101st fails");
    assert_eq!(
        err.downcast_ref::<AppError>().unwrap().code(),
        SHARE_WORKSPACE_QUOTA_EXCEEDED
    );
}

#[tokio::test]
async fn already_enabled_workspace_does_not_recheck_quota() {
    let store = Arc::new(MemoryShareStore::new());
    let owner_id = Uuid::new_v4();
    store.seed_owner_plan(owner_id, "free").await;
    let service = ShareService::new(store.clone());
    let auth = owner_auth(owner_id);

    let mut ids = Vec::new();
    for _ in 0..3 {
        let id = Uuid::new_v4();
        store.seed_workspace_owner(id, owner_id).await;
        service
            .update_access_level(&auth, &id.to_string(), "link")
            .await
            .unwrap();
        ids.push(id);
    }
    service
        .create_share_token(&auth, &ids[0].to_string(), AccessLevel::Read, None)
        .await
        .expect("token on already share_enabled workspace must not consume another slot");
    assert_eq!(store.count_enabled_for_owner(owner_id).await, 3);
}
