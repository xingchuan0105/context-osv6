mod support;

use std::sync::Arc;

use avrag_auth::{ActorId, AuthContext, OrgId, SubjectKind};
use avrag_share::{AccessLevel, ShareService};
use support::MemoryShareStore;
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
