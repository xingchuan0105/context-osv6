//! Contract tests for AppState domain faces after TN Wave 3.
//!
//! - Chat: `state.chat()`
//! - Notebooks/docs: `state.docs()`
//! - Admin API keys: `state.admin_api()`
//! - Share token helper still on AppState

use app::{AppConfig, AppState};
use common::CreateNotebookRequest;
use contracts::notebooks::CreateChatSessionRequest;

async fn memory_state() -> AppState {
    AppState::new(AppConfig::default())
}

async fn create_notebook(state: &AppState) -> contracts::notebooks::Notebook {
    state.docs()
        .create_notebook(CreateNotebookRequest {
            name: "delegate-contract".into(),
            description: String::new(),
        })
        .await
        .unwrap()
}

// ---------------------------------------------------------------------------
// Chat face: state.chat()
// ---------------------------------------------------------------------------

#[tokio::test]
async fn citation_lookup_missing_session_returns_session_not_found() {
    let state = memory_state().await;

    let err = state
        .chat()
        .lookup_citation("missing-session", 1, 1)
        .await
        .unwrap_err();

    assert_eq!(err.code(), "session_not_found");
    assert_eq!(err.http_status(), 404);
}

#[tokio::test]
async fn citation_lookup_unknown_message_returns_message_not_found() {
    let state = memory_state().await;
    let notebook = create_notebook(&state).await;
    let session = state
        .chat()
        .create_session(CreateChatSessionRequest {
            workspace_id: notebook.id,
            title: Some("citation contract".into()),
            agent_type: "chat".into(),
        })
        .await
        .unwrap();

    // Session exists; missing message id is a 404 on the message, not the session.
    let err = state
        .chat()
        .lookup_citation(&session.id, 999, 1)
        .await
        .unwrap_err();

    assert_eq!(err.code(), "message_not_found");
    assert_eq!(err.http_status(), 404);
}

#[tokio::test]
async fn citation_asset_missing_returns_not_found_in_memory_mode() {
    let state = memory_state().await;

    let err = state.chat().get_citation_asset("asset-1").await.unwrap_err();

    assert_eq!(err.code(), "asset_not_found");
    assert_eq!(err.http_status(), 404);
}

#[tokio::test]
async fn list_sessions_empty_for_new_notebook() {
    let state = memory_state().await;
    let notebook = create_notebook(&state).await;

    let sessions = state.chat().list_sessions(Some(&notebook.id)).await;

    assert!(sessions.is_empty());
}

#[tokio::test]
async fn execute_chat_empty_query_returns_validation_error() {
    let state = memory_state().await;

    let err = state.chat()
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
    let notebook = create_notebook(&state).await;

    let err = state
        .chat()
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
    let notebook = create_notebook(&state).await;

    let keys = state.admin_api().list_api_keys(&notebook.id).await.unwrap();

    assert!(keys.is_empty());
}

#[tokio::test]
async fn admin_create_share_token_succeeds_for_existing_notebook() {
    let state = memory_state().await;
    let notebook = create_notebook(&state).await;

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

    assert_eq!(err.code(), "notebook_not_found");
    assert_eq!(err.http_status(), 404);
}
