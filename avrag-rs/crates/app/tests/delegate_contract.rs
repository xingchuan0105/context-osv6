//! Contract tests for AppState domain faces after TN Wave 3.
//!
//! - Chat: `state.agent()`
//! - Workspaces/docs: `state.workspace()`
//! - Admin API keys: `state.admin_api()`
//! - Share token helper still on AppState

use app::{AppConfig, AppState};
use common::CreateWorkspaceRequest;
use contracts::workspaces::CreateChatSessionRequest;

async fn memory_state() -> AppState {
    AppState::new(AppConfig::default())
}

async fn create_workspace(state: &AppState) -> contracts::workspaces::Workspace {
    state.workspace()
        .create_workspace(CreateWorkspaceRequest {
            name: "delegate-contract".into(),
            description: String::new(),
        })
        .await
        .unwrap()
}

// ---------------------------------------------------------------------------
// Chat face: state.agent()
// ---------------------------------------------------------------------------

#[tokio::test]
async fn citation_lookup_missing_session_returns_session_not_found() {
    let state = memory_state().await;

    let err = state
        .agent()
        .lookup_citation("missing-session", 1, 1)
        .await
        .unwrap_err();

    assert_eq!(err.code(), "session_not_found");
    assert_eq!(err.http_status(), 404);
}

#[tokio::test]
async fn citation_lookup_unknown_message_returns_message_not_found() {
    let state = memory_state().await;
    let notebook = create_workspace(&state).await;
    let session = state
        .agent()
        .create_session(CreateChatSessionRequest {
            workspace_id: notebook.id,
            title: Some("citation contract".into()),
            agent_type: "chat".into(),
        })
        .await
        .unwrap();

    // Session exists; missing message id is a 404 on the message, not the session.
    let err = state
        .agent()
        .lookup_citation(&session.id, 999, 1)
        .await
        .unwrap_err();

    assert_eq!(err.code(), "message_not_found");
    assert_eq!(err.http_status(), 404);
}

#[tokio::test]
async fn citation_asset_missing_returns_not_found_in_memory_mode() {
    let state = memory_state().await;

    let err = state.agent().get_citation_asset("asset-1").await.unwrap_err();

    assert_eq!(err.code(), "asset_not_found");
    assert_eq!(err.http_status(), 404);
}

#[tokio::test]
async fn list_sessions_empty_for_new_workspace() {
    let state = memory_state().await;
    let notebook = create_workspace(&state).await;

    let sessions = state.agent().list_sessions(Some(&notebook.id)).await;

    assert!(sessions.is_empty());
}

#[tokio::test]
async fn execute_chat_empty_query_returns_validation_error() {
    let state = memory_state().await;

    let err = state.agent()
        .execute_chat(contracts::chat::ChatRequest {
            query: "   ".to_string(),
            workspace_id: None,
            session_id: None,
            agent_type: "chat".to_string(),
            source_type: None,
            source_token: None,
            doc_scope: Vec::new(),
            messages: Vec::new(),
            stream: false,
            debug: false,
            language: None,
            format_hint: None,
        })
        .await
        .unwrap_err();

    assert_eq!(err.code(), "query_required");
    assert_eq!(err.http_status(), 400);
}

#[tokio::test]
async fn execute_chat_memory_without_llm_returns_internal_error() {
    // Default memory AppState has no LLM client; contract is a clean error, not panic.
    let state = memory_state().await;
    let notebook = create_workspace(&state).await;

    let err = state
        .agent()
        .execute_chat(contracts::chat::ChatRequest {
            query: "hello from delegate contract".to_string(),
            workspace_id: Some(notebook.id.clone()),
            session_id: None,
            agent_type: "chat".to_string(),
            source_type: None,
            source_token: None,
            doc_scope: Vec::new(),
            messages: Vec::new(),
            stream: false,
            debug: false,
            language: None,
            format_hint: None,
        })
        .await
        .unwrap_err();

    assert_eq!(err.code(), "internal_error");
    assert!(
        err.message().to_ascii_lowercase().contains("llm"),
        "expected LLM configuration error, got: {}",
        err.message()
    );
}

// ---------------------------------------------------------------------------
// Admin delegates -> admin context
// ---------------------------------------------------------------------------

#[tokio::test]
async fn admin_list_api_keys_empty_for_valid_notebook() {
    let state = memory_state().await;
    let notebook = create_workspace(&state).await;

    let keys = state.admin_api().list_api_keys(&notebook.id).await.unwrap();

    assert!(keys.is_empty());
}

#[tokio::test]
async fn admin_create_share_token_succeeds_for_existing_notebook() {
    let state = memory_state().await;
    let notebook = create_workspace(&state).await;

    let response = state.share().create_share_token(&notebook.id).await.unwrap();

    assert!(
        response.share_token.starts_with("share_"),
        "expected share_ prefix, got {}",
        response.share_token
    );
}

#[tokio::test]
async fn admin_create_share_token_missing_notebook_returns_not_found() {
    let state = memory_state().await;

    let err = state
        .share()
        .create_share_token("missing-notebook")
        .await
        .unwrap_err();

    assert_eq!(err.code(), "workspace_not_found");
    assert_eq!(err.http_status(), 404);
}

// ---------------------------------------------------------------------------
// W2: Bound product faces (admin_ops / share access)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn admin_ops_without_actor_is_unauthorized() {
    use contracts::auth_runtime::{AuthContext, OrgId, SubjectKind};
    let state = memory_state().await;
    // Memory bootstrap attaches a default actor; strip it for this contract.
    let state = state.with_auth(AuthContext::new(OrgId::from(uuid::Uuid::nil()), SubjectKind::User));
    let err = state.admin_ops().list_feature_flags().await.unwrap_err();
    assert_eq!(err.http_status(), 401);
    assert_eq!(err.code(), "unauthorized");
}

#[tokio::test]
async fn share_check_access_memory_mode_allows() {
    let state = memory_state().await;
    let notebook = create_workspace(&state).await;
    let access = state.share().check_access(&notebook.id).await.unwrap();
    assert_ne!(access, avrag_share::AccessLevel::None);
}
