//! Memory-document-store adapter contract tests (review round-5 S5: these
//! live beside the adapter and go through the PORT methods only — no direct
//! `MemoryState` mutation, which would test nothing about the production
//! contract and fork memory semantics from PG).

use app_core::document_store::DocumentStorePort;
use app_core::{MemoryDocumentStore, MemoryState};
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

fn workspace_row(id: &Uuid, owner: &str) -> contracts::workspaces::Workspace {
    let now = common::now_rfc3339();
    contracts::workspaces::Workspace {
        id: id.to_string(),
        owner_user_id: owner.to_string(),
        owner_id: owner.to_string(),
        name: "ws".to_string(),
        title: "ws".to_string(),
        description: String::new(),
        created_at: now.clone(),
        updated_at: now,
        document_count: 0,
        status_summary: Default::default(),
        shared: false,
    }
}

async fn store_with_workspace() -> (MemoryDocumentStore, AuthContext, Uuid) {
    let owner = Uuid::new_v4();
    let auth = auth_for(owner);
    let workspace = Uuid::new_v4();
    let mut memory = MemoryState::default();
    memory
        .workspaces
        .insert(workspace.to_string(), workspace_row(&workspace, &owner.to_string()));
    // The workspace row is the one piece of seed state the port does not
    // create; it goes in before the store takes ownership of the state.
    let state = Arc::new(RwLock::new(memory));
    (MemoryDocumentStore::new(state), auth, workspace)
}

/// The workspace-binding version contract (PG parity): binding rows carry
/// their OWN id — never a copy of the artifact id — and surface the parse run
/// minted when the artifact reached Completed.
#[tokio::test]
async fn completed_workspace_binding_versions_follow_pg_contract() {
    let (store, auth, workspace) = store_with_workspace().await;

    // create → (worker flow) → completed: the ONLY parse-version write path
    // is the port-level status transition, matching PG's parse-run stamp.
    let doc: Document = store
        .create_document(&auth, workspace, "a.txt", 4, "text/plain")
        .await
        .unwrap();
    assert!(
        store
            .set_document_status(&auth, doc.id.parse().unwrap(), contracts::documents::DocumentStatus::Completed)
            .await
            .unwrap()
    );

    let versions = store
        .completed_workspace_binding_versions(&auth, workspace)
        .await
        .unwrap();
    assert_eq!(versions.len(), 1);
    let version = &versions[0];
    assert_ne!(
        version.binding_id, version.artifact_id,
        "binding id must be its own row id, not a copy of artifact_id"
    );
    assert_eq!(version.artifact_id, doc.id);
    assert!(
        version.parse_version.as_deref().unwrap_or_default().starts_with("parse-run-"),
        "completing the artifact must mint a parse run id on the binding: {version:?}"
    );

    // Pending artifacts (before completion) carry no version fact.
    let pending: Document = store
        .create_document(&auth, workspace, "b.txt", 4, "text/plain")
        .await
        .unwrap();
    let versions = store
        .completed_workspace_binding_versions(&auth, workspace)
        .await
        .unwrap();
    assert_eq!(versions.len(), 1, "pending artifact must stay out");
    assert_eq!(versions[0].artifact_id, doc.id);
    let _ = pending;
}

/// Session-file rows expose the binding id the client uses for DELETE, plus
/// the parse version written by the completion transition.
#[tokio::test]
async fn session_files_carry_binding_id_and_parse_version() {
    let owner = Uuid::new_v4();
    let auth = auth_for(owner);
    let workspace = Uuid::new_v4();
    let mut memory = MemoryState::default();
    memory
        .workspaces
        .insert(workspace.to_string(), workspace_row(&workspace, &owner.to_string()));
    let state = Arc::new(RwLock::new(memory));
    let store = MemoryDocumentStore::new(state.clone());

    let conversation = Uuid::new_v4();
    {
        let mut state = state.write().await;
        let now = common::now_rfc3339();
        state.sessions.insert(
            conversation.to_string(),
            contracts::workspaces::ChatSession {
                id: conversation.to_string(),
                owner_user_id: owner.to_string(),
                workspace_id: None,
                scope_kind: contracts::workspaces::ConversationScopeKind::Personal,
                workspace_name: None,
                title: None,
                agent_type: "chat".to_string(),
                model_role: "quick_chat".to_string(),
                pinned: false,
                created_at: now.clone(),
                updated_at: now,
            },
        );
    }
    let doc: Document = store
        .create_session_document(&auth, conversation, "s.txt", 4, "text/plain")
        .await
        .unwrap();
    store
        .set_document_status(&auth, doc.id.parse().unwrap(), contracts::documents::DocumentStatus::Completed)
        .await
        .unwrap();

    let files = store.list_session_files(&auth, conversation).await.unwrap();
    assert_eq!(files.len(), 1);
    let file = &files[0];
    assert_ne!(
        file.binding_id, file.document_id,
        "session binding id must be its own row id (the client DELETEs by it)"
    );
    assert!(
        files[0].parse_version.as_deref().unwrap_or_default().starts_with("parse-run-"),
        "completed session artifact must expose its parse run: {files:?}"
    );

    // Delete by the row's binding id — the same id the tray sends.
    let removed = store
        .delete_session_file_binding(&auth, conversation, files[0].binding_id.parse().unwrap())
        .await
        .unwrap();
    assert_eq!(removed.as_deref(), Some(doc.id.as_str()));
    let files = store.list_session_files(&auth, conversation).await.unwrap();
    assert!(files.is_empty());
}

/// Workspace deletion must drop its sessions' conversation bindings too —
/// leftover rows would keep artifacts alive and block the orphan sweep
/// (review round-5 P2: PG lifecycle parity).
#[tokio::test]
async fn delete_workspace_drops_session_bindings_and_orphans_session_only_artifacts() {
    let owner = Uuid::new_v4();
    let auth = auth_for(owner);
    let workspace = Uuid::new_v4();
    let mut memory = MemoryState::default();
    memory
        .workspaces
        .insert(workspace.to_string(), workspace_row(&workspace, &owner.to_string()));
    let state = Arc::new(RwLock::new(memory));
    let store = MemoryDocumentStore::new(state.clone());

    let conversation = Uuid::new_v4();
    {
        let mut state = state.write().await;
        let now = common::now_rfc3339();
        state.sessions.insert(
            conversation.to_string(),
            contracts::workspaces::ChatSession {
                id: conversation.to_string(),
                owner_user_id: owner.to_string(),
                workspace_id: Some(workspace.to_string()),
                scope_kind: contracts::workspaces::ConversationScopeKind::Workspace,
                workspace_name: Some("ws".to_string()),
                title: None,
                agent_type: "chat".to_string(),
                model_role: "agent".to_string(),
                pinned: false,
                created_at: now.clone(),
                updated_at: now,
            },
        );
    }
    let session_only: Document = store
        .create_session_document(&auth, conversation, "s.txt", 4, "text/plain")
        .await
        .unwrap();

    assert!(
        store.delete_workspace(&auth, workspace).await.unwrap()
    );
    {
        let state = state.read().await;
        assert!(
            state.conversation_document_bindings.is_empty(),
            "deleted session's conversation bindings must go with it: {:?}",
            state.conversation_document_bindings
        );
        assert!(
            state.workspace_document_bindings.is_empty(),
            "the workspace's own bindings must go too"
        );
        let stored = state.documents.get(&session_only.id).unwrap();
        assert!(
            matches!(stored.document.status, contracts::documents::DocumentStatus::Deleting),
            "zero-binding session artifact enters the deletion flow"
        );
    }
}