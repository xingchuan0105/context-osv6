//! Contract tests for AppState facade delegates (citation + admin surfaces).
//!
//! These tests exercise `AppState` public methods directly — the same entry
//! points used by transport-http handlers — without importing lib_impl internals.

use app::{AppConfig, AppState};
use common::{CreateDocumentRequest, CreateNotebookRequest};
use contracts::chat::ChatEvent;
use contracts::documents::DocumentStatus;
use contracts::notebooks::CreateChatSessionRequest;
use contracts::{ExecutePlanBudget, ExecutePlanItem, ExecutePlanRequest, ExecutePlanSummaryMode};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

async fn memory_state() -> AppState {
    AppState::new(AppConfig::default())
}

async fn create_notebook(state: &AppState) -> contracts::notebooks::Notebook {
    state
        .create_notebook(CreateNotebookRequest {
            name: "delegate-contract".into(),
            description: String::new(),
        })
        .await
        .unwrap()
}

// ---------------------------------------------------------------------------
// Citation delegates -> chat_ctx
// ---------------------------------------------------------------------------

#[tokio::test]
async fn citation_lookup_missing_session_returns_session_not_found() {
    let state = memory_state().await;

    let err = state
        .lookup_citation("missing-session", 1, 1)
        .await
        .unwrap_err();

    assert_eq!(err.code(), "session_not_found");
    assert_eq!(err.http_status(), 404);
}

#[tokio::test]
async fn citation_lookup_session_without_messages_returns_session_not_found() {
    let state = memory_state().await;
    let notebook = create_notebook(&state).await;
    let session = state
        .create_session(CreateChatSessionRequest {
            notebook_id: notebook.id,
            title: Some("citation contract".into()),
            agent_type: "chat".into(),
        })
        .await
        .unwrap();

    // Memory backend stores messages in a separate map; a fresh session has no entry yet.
    let err = state
        .lookup_citation(&session.id, 999, 1)
        .await
        .unwrap_err();

    assert_eq!(err.code(), "session_not_found");
    assert_eq!(err.http_status(), 404);
}

#[tokio::test]
async fn citation_asset_memory_mode_requires_postgres_backend() {
    let state = memory_state().await;

    let err = state.get_citation_asset("asset-1").await.unwrap_err();

    assert_eq!(err.code(), "internal_error");
    assert!(
        err.message().contains("postgres"),
        "expected postgres requirement message, got: {}",
        err.message()
    );
}

// ---------------------------------------------------------------------------
// Chat delegates -> chat_ctx
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_sessions_empty_for_new_notebook() {
    let state = memory_state().await;
    let notebook = create_notebook(&state).await;

    let sessions = state.list_sessions(Some(&notebook.id)).await;

    assert!(sessions.is_empty());
}

#[tokio::test]
async fn execute_chat_empty_query_returns_validation_error() {
    let state = memory_state().await;

    let err = state
        .execute_chat(contracts::chat::ChatRequest {
            query: "   ".to_string(),
            notebook_id: None,
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
async fn execute_chat_memory_rag_mode_returns_answer() {
    let state = memory_state().await;
    let notebook = create_notebook(&state).await;
    let upload = state
        .create_document_upload(
            &notebook.id,
            CreateDocumentRequest {
                filename: "notes.txt".to_string(),
                file_size: 12,
                mime_type: "text/plain".to_string(),
            },
        )
        .await
        .unwrap();
    state
        .put_uploaded_document(&upload.document_id, b"hello memory".to_vec())
        .await
        .unwrap();
    state
        .transition_document_status(&upload.document_id, DocumentStatus::Completed)
        .await
        .unwrap();

    let response = state
        .execute_chat(contracts::chat::ChatRequest {
            query: "summarize notes".to_string(),
            notebook_id: Some(notebook.id.clone()),
            session_id: None,
            agent_type: "rag".to_string(),
            source_type: None,
            source_token: None,
            doc_scope: vec![upload.document_id.clone()],
            messages: Vec::new(),
            stream: false,
            debug: false,
            language: None,
            format_hint: None,
        })
        .await
        .unwrap();

    assert!(!response.answer.is_empty());
    assert_eq!(response.agent_type, "rag");
}

#[tokio::test]
async fn execute_chat_stream_memory_rag_emits_done_event() {
    let state = memory_state().await;
    let notebook = create_notebook(&state).await;
    let upload = state
        .create_document_upload(
            &notebook.id,
            CreateDocumentRequest {
                filename: "stream.txt".to_string(),
                file_size: 12,
                mime_type: "text/plain".to_string(),
            },
        )
        .await
        .unwrap();
    state
        .put_uploaded_document(&upload.document_id, b"stream notes".to_vec())
        .await
        .unwrap();
    state
        .transition_document_status(&upload.document_id, DocumentStatus::Completed)
        .await
        .unwrap();

    let (tx, mut rx) = mpsc::unbounded_channel();
    state
        .execute_chat_stream(
            contracts::chat::ChatRequest {
                query: "stream summarize".to_string(),
                notebook_id: Some(notebook.id.clone()),
                session_id: None,
                agent_type: "rag".to_string(),
                source_type: None,
                source_token: None,
                doc_scope: vec![upload.document_id.clone()],
                messages: Vec::new(),
                stream: true,
                debug: false,
                language: None,
                format_hint: None,
            },
            "delegate-contract-stream".to_string(),
            tx,
            CancellationToken::new(),
        )
        .await
        .unwrap();

    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }

    assert!(
        events
            .iter()
            .any(|event| matches!(event, ChatEvent::Done { .. })),
        "expected Done event, got {events:?}"
    );
}

#[tokio::test]
async fn execute_rag_execute_plan_returns_gone() {
    let state = memory_state().await;
    let err = state
        .execute_rag_execute_plan(ExecutePlanRequest {
            plan_version: "rag-execute-v1".to_string(),
            doc_scope: vec!["00000000-0000-0000-0000-000000000001".to_string()],
            items: vec![ExecutePlanItem {
                priority: 1.0,
                query: Some("plan".to_string()),
                bm25_terms: None,
            }],
            summary_mode: ExecutePlanSummaryMode::All,
            budget: Some(ExecutePlanBudget {
                total_candidate_budget: Some(4),
                final_chunk_budget: Some(1),
                graph_hop_limit: None,
                graph_fan_out_limit: None,
            }),
            channel_budget: None,
            query_entities: Vec::new(),
            graph_hints: Vec::new(),
            placeholder_triplets: Vec::new(),
            trace: None,
        })
        .await
        .unwrap_err();

    assert_eq!(err.code(), "execute_plan_gone");
    assert_eq!(err.http_status(), 410);
}

// ---------------------------------------------------------------------------
// Admin delegates -> admin context
// ---------------------------------------------------------------------------

#[tokio::test]
async fn admin_list_api_keys_empty_for_valid_notebook() {
    let state = memory_state().await;
    let notebook = create_notebook(&state).await;

    let keys = state.list_api_keys(&notebook.id).await.unwrap();

    assert!(keys.is_empty());
}

#[tokio::test]
async fn admin_create_share_token_succeeds_for_existing_notebook() {
    let state = memory_state().await;
    let notebook = create_notebook(&state).await;

    let response = state.create_share_token(&notebook.id).await.unwrap();

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
        .create_share_token("missing-notebook")
        .await
        .unwrap_err();

    assert_eq!(err.code(), "notebook_not_found");
    assert_eq!(err.http_status(), 404);
}
