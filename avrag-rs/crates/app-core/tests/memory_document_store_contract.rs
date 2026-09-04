//! Memory-document-store adapter contract tests (review round-5 S5 / round-6
//! Spec-5): these live beside the adapter and go through PORT methods only —
//! no direct `MemoryState` seeding or internal reads. Workspaces are created
//! via `DocumentStorePort::create_workspace`, conversations via
//! `SessionPort::create_session` (shared `MemoryState` behind the two
//! adapters), and every assertion reads a public query port.

use app_core::chat_persistence::SessionPort;
use app_core::document_store::DocumentStorePort;
use app_core::{MemoryChatPersistence, MemoryDocumentStore, MemoryState};
use common::Document;
use std::sync::Arc;
use tokio::sync::RwLock;

use contracts::auth_runtime::{ActorId, AuthContext, SubjectKind, UserId};
use uuid::Uuid;

fn auth_for(user_id: Uuid) -> AuthContext {
    AuthContext::new(UserId::from(user_id), SubjectKind::User)
        .with_actor_id(ActorId::new(user_id))
        .with_request_id("memory-contract")
}

async fn port_harness() -> (MemoryDocumentStore, MemoryChatPersistence, AuthContext) {
    let auth = auth_for(Uuid::new_v4());
    let memory = Arc::new(RwLock::new(MemoryState::default()));
    let store = MemoryDocumentStore::new(memory.clone());
    let chat = MemoryChatPersistence::new(memory);
    (store, chat, auth)
}

/// The workspace-binding row contract: binding rows carry their OWN id —
/// never a copy of the artifact id. Seeded through the ports (workspace via
/// `create_workspace`, artifact via `create_document`).
#[tokio::test]
async fn completed_workspace_binding_versions_carry_own_binding_id() {
    let (store, _chat, auth) = port_harness().await;

    let workspace = store.create_workspace(&auth, "ws", "").await.unwrap();
    let workspace_id = workspace.id.parse().unwrap();
    let doc: Document = store
        .create_document(&auth, workspace_id, "a.txt", 4, "text/plain")
        .await
        .unwrap();

    let versions = store
        .completed_workspace_binding_versions(&auth, workspace_id)
        .await
        .unwrap();
    // Pending artifact: a binding row exists but is withheld until completion.
    assert!(versions.is_empty(), "pending artifact must stay out");

    // A second artifact still pending: the completed filter must be per-row.
    let _pending: Document = store
        .create_document(&auth, workspace_id, "b.txt", 4, "text/plain")
        .await
        .unwrap();

    let listed = store
        .list_documents(&auth, Some(workspace_id), None)
        .await
        .unwrap();
    assert_eq!(listed.len(), 2, "both artifacts exist pre-completion");

    let _ = store
        .set_document_status(
            &auth,
            doc.id.parse().unwrap(),
            contracts::documents::DocumentStatus::Completed,
        )
        .await
        .unwrap();
    let versions = store
        .completed_workspace_binding_versions(&auth, workspace_id)
        .await
        .unwrap();
    assert_eq!(versions.len(), 1, "only the completed artifact surfaces");
    assert_ne!(
        versions[0].binding_id, versions[0].artifact_id,
        "binding id must be its own row id, not a copy of artifact_id"
    );
    assert_eq!(versions[0].artifact_id, doc.id);
}

/// Session-file rows expose the binding id the client uses for DELETE. The
/// conversation is created through `SessionPort::create_session` — the same
/// port the product pipeline uses.
#[tokio::test]
async fn session_files_carry_binding_id_and_delete_by_row() {
    let (store, chat, auth) = port_harness().await;

    let conversation = chat
        .create_session(&auth, None, None, "chat", "quick_chat")
        .await
        .unwrap();
    let conversation_id = Uuid::parse_str(&conversation.id).unwrap();

    let doc: Document = store
        .create_session_document(&auth, conversation_id, "s.txt", 4, "text/plain")
        .await
        .unwrap();

    let files = store
        .list_session_files(&auth, conversation_id)
        .await
        .unwrap();
    assert_eq!(files.len(), 1);
    assert_ne!(
        files[0].binding_id, files[0].document_id,
        "session binding id must be its own row id (the client DELETEs by it)"
    );

    // Delete by the row's binding id — the same id the tray sends.
    let removed = store
        .delete_session_file_binding(&auth, conversation_id, files[0].binding_id.parse().unwrap())
        .await
        .unwrap();
    assert_eq!(removed.as_deref(), Some(doc.id.as_str()));
    let files = store
        .list_session_files(&auth, conversation_id)
        .await
        .unwrap();
    assert!(files.is_empty());
}

/// Explicit orphan GC must report the shared unbound transition itself: a
/// bound artifact is refused, the first unbound call marks it, and retries are
/// idempotent once deletion has started.
#[tokio::test]
async fn explicit_unbound_delete_reports_shared_transition() {
    let (store, chat, auth) = port_harness().await;

    let conversation = chat
        .create_session(&auth, None, None, "chat", "quick_chat")
        .await
        .unwrap();
    let conversation_id = Uuid::parse_str(&conversation.id).unwrap();
    let doc = store
        .create_session_document(&auth, conversation_id, "orphan.txt", 4, "text/plain")
        .await
        .unwrap();
    let document_id = Uuid::parse_str(&doc.id).unwrap();

    assert!(
        !store
            .delete_document_if_unbound(&auth, document_id)
            .await
            .unwrap(),
        "a conversation binding must keep the artifact alive"
    );

    let binding = store
        .list_session_files(&auth, conversation_id)
        .await
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    store
        .delete_session_file_binding(
            &auth,
            conversation_id,
            Uuid::parse_str(&binding.binding_id).unwrap(),
        )
        .await
        .unwrap();

    assert!(
        store
            .delete_document_if_unbound(&auth, document_id)
            .await
            .unwrap(),
        "the first unbound call must perform the Deleting transition"
    );
    assert!(
        !store
            .delete_document_if_unbound(&auth, document_id)
            .await
            .unwrap(),
        "an already-deleting artifact must not report a second transition"
    );

    let states = store
        .get_document_scope_states(&auth, &[document_id])
        .await
        .unwrap();
    assert!(matches!(
        states.as_slice(),
        [state] if state.status == contracts::documents::DocumentStatus::Deleting
    ));
}

/// Workspace deletion must drop its sessions' conversation bindings too —
/// leftover rows would keep artifacts alive and block the orphan sweep
/// (review round-5 P2: PG lifecycle parity). The artifact's state is checked
/// via the public `list_documents` port (deleting artifacts stay listed but
/// carry the Deleting status).
#[tokio::test]
async fn delete_workspace_drops_session_bindings_and_orphans_session_only_artifacts() {
    let (store, chat, auth) = port_harness().await;

    let workspace = store.create_workspace(&auth, "ws", "").await.unwrap();
    let workspace_id = Uuid::parse_str(&workspace.id).unwrap();
    let conversation = chat
        .create_session(&auth, Some(workspace_id), None, "chat", "agent")
        .await
        .unwrap();
    let conversation_id = Uuid::parse_str(&conversation.id).unwrap();

    let session_only: Document = store
        .create_session_document(&auth, conversation_id, "s.txt", 4, "text/plain")
        .await
        .unwrap();

    assert!(store.delete_workspace(&auth, workspace_id).await.unwrap());

    // The conversation went with the workspace (SessionPort view).
    let sessions = chat.list_sessions(&auth, Some(workspace_id)).await.unwrap();
    assert!(
        sessions.is_empty(),
        "workspace delete cascades its sessions"
    );
    // The artifact's conversation binding is gone → list_session_files is empty.
    let files = store
        .list_session_files(&auth, conversation_id)
        .await
        .unwrap();
    assert!(files.is_empty());
    // Workspace bindings are gone → completed_workspace_binding_versions empty.
    let versions = store
        .completed_workspace_binding_versions(&auth, workspace_id)
        .await
        .unwrap();
    assert!(versions.is_empty());
    // The zero-binding session-only artifact enters the deletion flow —
    // observable via the public scope-states port (status Deleting;
    // list_documents excludes deleting rows by contract).
    let scope_states = store
        .get_document_scope_states(&auth, &[session_only.id.parse().unwrap()])
        .await
        .unwrap();
    assert_eq!(scope_states.len(), 1);
    assert!(
        matches!(
            scope_states[0].status,
            contracts::documents::DocumentStatus::Deleting
        ),
        "zero-binding session artifact must enter deletion: {:?}",
        scope_states[0]
    );
}
