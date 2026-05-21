//! Route handler implementations for the transport-http crate.

use app::AppState;
use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header},
    response::{
        IntoResponse, Response, Sse,
        sse::{Event, KeepAlive},
    },
};
use common::{
    AddUrlSourceRequest, AppError, ChatRequest, CitationLookupRequest, CreateChatSessionRequest,
    CreateDocumentRequest, CreateNotebookNoteRequest, CreateNotebookRequest, ExecutePlanRequest,
    NotebookListResponse, NotebookResponse,
    RuntimeExecuteRequest, UpdateChatSessionRequest, UpdateDocumentRequest,
    UpdateNotebookNoteRequest, UpdateNotebookRequest,
};
use contracts::chat::ChatEvent;
use std::{convert::Infallible, time::Duration};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::RequestState;

// ---------------------------------------------------------------------------
// Error helpers
// ---------------------------------------------------------------------------

/// Convert an [`AppError`] into an HTTP response with a typed JSON body.
pub(crate) fn app_error_response(e: AppError) -> Response {
    let status = StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut body = serde_json::json!({
        "error": e.code(),
        "message": e.message(),
    });
    let retry_after = e.retry_after_secs();
    if let Some(secs) = retry_after {
        body["retry_after_secs"] = serde_json::json!(secs);
    }
    let mut response = (status, Json(body)).into_response();
    if status == StatusCode::TOO_MANY_REQUESTS {
        if let Some(secs) = retry_after {
            response.headers_mut().insert(
                header::RETRY_AFTER,
                HeaderValue::from(secs as u64),
            );
        }
    }
    response
}

/// Return a JSON error response.
pub(crate) fn error_response(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({
            "error": code,
            "message": message,
        })),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Notebook handlers
// ---------------------------------------------------------------------------

#[utoipa::path(
    get,
    path = "/api/v1/notebooks",
    responses(
        (status = 200, description = "List all notebooks", body = NotebookListResponse)
    ),
    tag = "notebooks"
)]
pub(crate) async fn list_notebooks(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Response {
    let notebooks = state.list_notebooks().await;
    (
        StatusCode::OK,
        Json(common::NotebookListResponse { notebooks }),
    )
        .into_response()
}

#[utoipa::path(
    get,
    path = "/api/v1/notebooks/{id}",
    responses(
        (status = 200, description = "Get a notebook by ID", body = NotebookResponse),
        (status = 404, description = "Notebook not found")
    ),
    params(
        ("id" = String, Path, description = "Notebook ID")
    ),
    tag = "notebooks"
)]
pub(crate) async fn get_notebook(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(id): Path<String>,
) -> Response {
    match state.get_notebook(&id).await {
        Some(nb) => {
            state
                .record_product_event_if_available(
                    analytics::ProductEventName::NotebookOpened,
                    analytics::Surface::Workspace,
                    analytics::ResultTag::Success,
                    None,
                    Uuid::parse_str(&nb.id).ok(),
                    serde_json::json!({
                        "notebook_id": nb.id.clone(),
                    }),
                )
                .await;
            (
                StatusCode::OK,
                Json(common::NotebookResponse { notebook: nb }),
            )
                .into_response()
        }
        None => error_response(
            StatusCode::NOT_FOUND,
            "not_found",
            &format!("Notebook {id} not found"),
        ),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/notebooks",
    request_body = CreateNotebookRequest,
    responses(
        (status = 201, description = "Notebook created", body = NotebookResponse)
    ),
    tag = "notebooks"
)]
pub(crate) async fn create_notebook(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(req): Json<CreateNotebookRequest>,
) -> Response {
    match state.create_notebook(req).await {
        Ok(nb) => (
            StatusCode::CREATED,
            Json(common::NotebookResponse { notebook: nb }),
        )
            .into_response(),
        Err(e) => app_error_response(e),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/notebooks/{id}",
    request_body = UpdateNotebookRequest,
    responses(
        (status = 200, description = "Notebook updated", body = NotebookResponse),
        (status = 404, description = "Notebook not found")
    ),
    params(
        ("id" = String, Path, description = "Notebook ID")
    ),
    tag = "notebooks"
)]
pub(crate) async fn update_notebook(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateNotebookRequest>,
) -> Response {
    match state.update_notebook(&id, req).await {
        Ok(nb) => (
            StatusCode::OK,
            Json(common::NotebookResponse { notebook: nb }),
        )
            .into_response(),
        Err(e) => app_error_response(e),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/notebooks/{id}",
    responses(
        (status = 200, description = "Notebook deleted"),
        (status = 404, description = "Notebook not found")
    ),
    params(
        ("id" = String, Path, description = "Notebook ID")
    ),
    tag = "notebooks"
)]
pub(crate) async fn delete_notebook(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(id): Path<String>,
) -> Response {
    match state.delete_notebook(&id).await {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "deleted"})),
        )
            .into_response(),
        Err(e) => app_error_response(e),
    }
}

fn note_preview(content: &str) -> String {
    let collapsed = content
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();
    if collapsed.chars().count() <= 140 {
        return collapsed;
    }
    let preview = collapsed.chars().take(140).collect::<String>();
    format!("{preview}...")
}

fn normalize_note_title(title: Option<String>) -> String {
    title
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Untitled note".to_string())
}

fn slugify_note_filename(title: &str) -> String {
    let slug = title
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let collapsed = slug
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if collapsed.is_empty() {
        "note".to_string()
    } else {
        collapsed
    }
}

fn notebook_note_from_pref(note: &common::NotebookNotePreference) -> common::NotebookNote {
    common::NotebookNote {
        id: note.note_id.clone(),
        notebook_id: note.notebook_id.clone(),
        title: note.title.clone(),
        content: note.content.clone(),
        preview: note_preview(&note.content),
        created_at: note.created_at.clone(),
        updated_at: note.updated_at.clone(),
        promoted_document_id: note.promoted_document_id.clone(),
        promoted_at: note.promoted_at.clone(),
    }
}

fn migrate_workspace_draft_to_note(
    preferences: &mut common::UserPreferences,
    notebook_id: &str,
) -> bool {
    let has_notes = preferences
        .dashboard
        .notebook_notes
        .iter()
        .any(|note| note.notebook_id == notebook_id);
    if has_notes {
        return false;
    }

    let Some(index) = preferences
        .dashboard
        .workspace_drafts
        .iter()
        .position(|draft| draft.notebook_id == notebook_id && !draft.notes.trim().is_empty())
    else {
        return false;
    };

    let legacy = preferences.dashboard.workspace_drafts.remove(index);
    let now = chrono::Utc::now().to_rfc3339();
    preferences
        .dashboard
        .notebook_notes
        .push(common::NotebookNotePreference {
            note_id: Uuid::new_v4().to_string(),
            notebook_id: notebook_id.to_string(),
            title: "Imported Notes".to_string(),
            content: legacy.notes,
            created_at: now.clone(),
            updated_at: now,
            promoted_document_id: None,
            promoted_at: None,
        });
    true
}

async fn load_notebook_notes(
    state: &AppState,
    notebook_id: &str,
) -> Result<Vec<common::NotebookNote>, AppError> {
    let mut preferences = state.current_user_preferences().await?;
    let migrated = migrate_workspace_draft_to_note(&mut preferences, notebook_id);
    if migrated {
        state.save_current_user_preferences(&preferences).await?;
    }

    let mut notes = preferences
        .dashboard
        .notebook_notes
        .iter()
        .filter(|note| note.notebook_id == notebook_id)
        .map(notebook_note_from_pref)
        .collect::<Vec<_>>();
    notes.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.title.cmp(&right.title))
    });
    Ok(notes)
}

fn pinned_source_count(preferences: &common::UserPreferences, notebook_id: &str) -> i64 {
    preferences
        .dashboard
        .workspace_preferences
        .iter()
        .find(|pref| pref.notebook_id == notebook_id)
        .map(|pref| pref.pinned_source_ids.len() as i64)
        .unwrap_or(0)
}

pub(crate) async fn list_notebook_notes_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
) -> Response {
    if state.get_notebook(&notebook_id).await.is_none() {
        return error_response(StatusCode::NOT_FOUND, "not_found", "Notebook not found");
    }

    match load_notebook_notes(&state, &notebook_id).await {
        Ok(notes) => (
            StatusCode::OK,
            Json(common::NotebookNoteListResponse { notes }),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn get_notebook_note_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((notebook_id, note_id)): Path<(String, String)>,
) -> Response {
    if state.get_notebook(&notebook_id).await.is_none() {
        return error_response(StatusCode::NOT_FOUND, "not_found", "Notebook not found");
    }

    match load_notebook_notes(&state, &notebook_id).await {
        Ok(notes) => match notes.into_iter().find(|note| note.id == note_id) {
            Some(note) => {
                (StatusCode::OK, Json(common::NotebookNoteResponse { note })).into_response()
            }
            None => error_response(StatusCode::NOT_FOUND, "not_found", "Note not found"),
        },
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn create_notebook_note_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
    Json(req): Json<CreateNotebookNoteRequest>,
) -> Response {
    if state.get_notebook(&notebook_id).await.is_none() {
        return error_response(StatusCode::NOT_FOUND, "not_found", "Notebook not found");
    }

    let mut preferences = match state.current_user_preferences().await {
        Ok(preferences) => preferences,
        Err(error) => return app_error_response(error),
    };
    let now = chrono::Utc::now().to_rfc3339();
    let note = common::NotebookNotePreference {
        note_id: Uuid::new_v4().to_string(),
        notebook_id: notebook_id.clone(),
        title: normalize_note_title(req.title),
        content: req.content.unwrap_or_default(),
        created_at: now.clone(),
        updated_at: now,
        promoted_document_id: None,
        promoted_at: None,
    };
    preferences.dashboard.notebook_notes.push(note.clone());

    match state.save_current_user_preferences(&preferences).await {
        Ok(_) => (
            StatusCode::CREATED,
            Json(common::NotebookNoteResponse {
                note: notebook_note_from_pref(&note),
            }),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn update_notebook_note_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((notebook_id, note_id)): Path<(String, String)>,
    Json(req): Json<UpdateNotebookNoteRequest>,
) -> Response {
    if state.get_notebook(&notebook_id).await.is_none() {
        return error_response(StatusCode::NOT_FOUND, "not_found", "Notebook not found");
    }

    let mut preferences = match state.current_user_preferences().await {
        Ok(preferences) => preferences,
        Err(error) => return app_error_response(error),
    };
    let migrated = migrate_workspace_draft_to_note(&mut preferences, &notebook_id);
    let Some(note) = preferences
        .dashboard
        .notebook_notes
        .iter_mut()
        .find(|note| note.notebook_id == notebook_id && note.note_id == note_id)
    else {
        if migrated {
            let _ = state.save_current_user_preferences(&preferences).await;
        }
        return error_response(StatusCode::NOT_FOUND, "not_found", "Note not found");
    };

    if let Some(title) = req.title {
        note.title = normalize_note_title(Some(title));
    }
    if let Some(content) = req.content {
        note.content = content;
    }
    note.updated_at = chrono::Utc::now().to_rfc3339();
    let response = notebook_note_from_pref(note);

    match state.save_current_user_preferences(&preferences).await {
        Ok(_) => (
            StatusCode::OK,
            Json(common::NotebookNoteResponse { note: response }),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn delete_notebook_note_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((notebook_id, note_id)): Path<(String, String)>,
) -> Response {
    if state.get_notebook(&notebook_id).await.is_none() {
        return error_response(StatusCode::NOT_FOUND, "not_found", "Notebook not found");
    }

    let mut preferences = match state.current_user_preferences().await {
        Ok(preferences) => preferences,
        Err(error) => return app_error_response(error),
    };
    let before = preferences.dashboard.notebook_notes.len();
    preferences
        .dashboard
        .notebook_notes
        .retain(|note| !(note.notebook_id == notebook_id && note.note_id == note_id));
    if before == preferences.dashboard.notebook_notes.len() {
        return error_response(StatusCode::NOT_FOUND, "not_found", "Note not found");
    }

    match state.save_current_user_preferences(&preferences).await {
        Ok(_) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn promote_notebook_note_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((notebook_id, note_id)): Path<(String, String)>,
) -> Response {
    if state.get_notebook(&notebook_id).await.is_none() {
        return error_response(StatusCode::NOT_FOUND, "not_found", "Notebook not found");
    }

    let mut preferences = match state.current_user_preferences().await {
        Ok(preferences) => preferences,
        Err(error) => return app_error_response(error),
    };
    let migrated = migrate_workspace_draft_to_note(&mut preferences, &notebook_id);
    let Some(note) = preferences
        .dashboard
        .notebook_notes
        .iter_mut()
        .find(|note| note.notebook_id == notebook_id && note.note_id == note_id)
    else {
        if migrated {
            let _ = state.save_current_user_preferences(&preferences).await;
        }
        return error_response(StatusCode::NOT_FOUND, "not_found", "Note not found");
    };

    if note.content.trim().is_empty() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "Cannot promote an empty note",
        );
    }

    let markdown = format!("# {}\n\n{}\n", note.title, note.content);
    let filename = format!("{}.md", slugify_note_filename(&note.title));
    let upload = match state
        .create_document_upload(
            &notebook_id,
            CreateDocumentRequest {
                filename,
                file_size: markdown.len() as u64,
                mime_type: "text/markdown".to_string(),
            },
        )
        .await
    {
        Ok(upload) => upload,
        Err(error) => return app_error_response(error),
    };

    if let Err(error) = state
        .put_uploaded_document(&upload.document_id, markdown.into_bytes())
        .await
    {
        return app_error_response(error);
    }
    if let Err(error) = state.complete_document_upload(&upload.document_id).await {
        return app_error_response(error);
    }

    let promoted_at = chrono::Utc::now().to_rfc3339();
    note.promoted_document_id = Some(upload.document_id.clone());
    note.promoted_at = Some(promoted_at);
    note.updated_at = chrono::Utc::now().to_rfc3339();
    let response_note = notebook_note_from_pref(note);

    match state.save_current_user_preferences(&preferences).await {
        Ok(_) => (
            StatusCode::OK,
            Json(common::PromoteNotebookNoteResponse {
                note: response_note,
                source_id: upload.document_id,
            }),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn get_notebook_analysis_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
) -> Response {
    let Some(notebook) = state.get_notebook(&notebook_id).await else {
        return error_response(StatusCode::NOT_FOUND, "not_found", "Notebook not found");
    };

    let sources = state.list_documents(Some(&notebook_id), None).await;
    let sessions = state.list_sessions(Some(&notebook_id)).await;
    let preferences = match state.current_user_preferences().await {
        Ok(preferences) => preferences,
        Err(error) => return app_error_response(error),
    };
    let notes = match load_notebook_notes(&state, &notebook_id).await {
        Ok(notes) => notes,
        Err(error) => return app_error_response(error),
    };

    let ready_sources = sources
        .iter()
        .filter(|source| matches!(source.status.as_str(), "ready" | "completed"))
        .count() as i64;
    let failed_sources = sources
        .iter()
        .filter(|source| matches!(source.status.as_str(), "failed" | "error"))
        .count() as i64;
    let processing_sources = sources.len() as i64 - ready_sources - failed_sources;
    let pinned_sources = pinned_source_count(&preferences, &notebook_id);
    let latest_session = sessions
        .iter()
        .max_by(|left, right| left.updated_at.cmp(&right.updated_at));
    let promoted_notes = notes
        .iter()
        .filter(|note| note.promoted_document_id.is_some())
        .count() as i64;
    let latest_note_update = notes.iter().map(|note| note.updated_at.clone()).max();

    let member_count = if let Some(pg) = state.pg() {
        avrag_share::handle_list_members(state.auth().clone(), notebook_id.clone(), pg)
            .await
            .map(|members| members.len() as i64)
            .unwrap_or(0)
    } else {
        0
    };
    let share_enabled = if let Some(pg) = state.pg() {
        avrag_share::handle_get_share_settings(state.auth().clone(), notebook_id.clone(), pg)
            .await
            .map(|settings| {
                settings
                    .share_tokens
                    .iter()
                    .any(|token| token.revoked_at.is_none() && !token.token.trim().is_empty())
                    && !settings.access_level.eq_ignore_ascii_case("private")
            })
            .unwrap_or(false)
    } else {
        false
    };
    let active_api_key_count = state
        .list_api_keys(&notebook_id)
        .await
        .map(|items| items.into_iter().filter(|item| item.is_active).count() as i64)
        .unwrap_or(0);

    let mut alerts = Vec::new();
    if ready_sources == 0 {
        alerts.push(common::NotebookAnalysisAlert {
            level: "warning".to_string(),
            code: "no_ready_sources".to_string(),
            message: "No ready sources are available for RAG chat.".to_string(),
        });
    }
    if failed_sources > 0 {
        alerts.push(common::NotebookAnalysisAlert {
            level: "warning".to_string(),
            code: "failed_sources".to_string(),
            message: format!("{failed_sources} sources need attention or reindexing."),
        });
    }
    if sessions.is_empty() {
        alerts.push(common::NotebookAnalysisAlert {
            level: "info".to_string(),
            code: "no_threads".to_string(),
            message: "This notebook does not have any threads yet.".to_string(),
        });
    }
    if !notes.is_empty() && promoted_notes == 0 {
        alerts.push(common::NotebookAnalysisAlert {
            level: "info".to_string(),
            code: "notes_not_promoted".to_string(),
            message: "Notes exist, but none have been promoted into shared sources yet."
                .to_string(),
        });
    }

    (
        StatusCode::OK,
        Json(common::NotebookAnalysisResponse {
            overview: common::NotebookAnalysisOverview {
                title: notebook.title,
                description: notebook.description,
                updated_at: notebook.updated_at,
                document_count: notebook.document_count,
            },
            sources: common::NotebookAnalysisSources {
                total: sources.len() as i64,
                ready: ready_sources,
                processing: processing_sources.max(0),
                failed: failed_sources,
                selected: 0,
                pinned: pinned_sources,
            },
            threads: common::NotebookAnalysisThreads {
                total: sessions.len() as i64,
                pinned: sessions.iter().filter(|session| session.pinned).count() as i64,
                latest_activity_at: latest_session.map(|session| session.updated_at.clone()),
                latest_mode: latest_session.map(|session| session.agent_type.clone()),
            },
            notes: common::NotebookAnalysisNotes {
                total: notes.len() as i64,
                latest_edited_at: latest_note_update,
                promoted_to_source: promoted_notes,
            },
            access: common::NotebookAnalysisAccess {
                share_enabled,
                member_count,
                active_api_key_count,
            },
            alerts,
        }),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Chat handler
// ---------------------------------------------------------------------------

pub(crate) async fn rag_execute_plan_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    payload: Result<Json<ExecutePlanRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Json(req) = match payload {
        Ok(payload) => payload,
        Err(error) => {
            return app_error_response(AppError::validation(
                "invalid_execute_plan",
                format!("invalid execute-plan JSON: {error}"),
            ));
        }
    };
    match state.execute_rag_execute_plan(req).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn runtime_execute_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    payload: Result<Json<RuntimeExecuteRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Json(req) = match payload {
        Ok(payload) => payload,
        Err(error) => {
            return app_error_response(AppError::validation(
                "invalid_runtime_execute",
                format!("invalid runtime execute JSON: {error}"),
            ));
        }
    };
    match state.execute_runtime_tools(req).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => app_error_response(error),
    }
}

#[tracing::instrument(skip(state, headers), fields(agent_type = %req.agent_type, request_id = tracing::field::Empty))]
pub(crate) async fn chat_post_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    headers: HeaderMap,
    Json(req): Json<ChatRequest>,
) -> Response {
    let should_stream = req.stream || accepts_sse(&headers);
    let source_type = req.source_type.clone();
    let notebook_id = req
        .notebook_id
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok());
    let agent_type = req.agent_type.clone();
    let query_len = req.query.len();
    let surface = if source_type.as_deref() == Some("share") {
        analytics::Surface::SharedKb
    } else {
        analytics::Surface::Workspace
    };
    let request_id = state
        .auth()
        .request_id()
        .map(str::to_string)
        .or_else(|| {
            headers
                .get("x-request-id")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string)
        })
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    tracing::Span::current().record("request_id", &request_id);

    let started_event = if source_type.as_deref() == Some("share") {
        analytics::ProductEventName::SharedKbChatStarted
    } else if agent_type == "search" {
        analytics::ProductEventName::SearchStarted
    } else {
        analytics::ProductEventName::ChatStarted
    };
    state
        .record_product_event_if_available(
            started_event,
            surface,
            analytics::ResultTag::Success,
            None,
            notebook_id,
            serde_json::json!({
                "agent_type": agent_type,
                "query_length": query_len,
                "stream": should_stream,
            }),
        )
        .await;

    if should_stream {
        return chat_live_stream_response(
            state,
            req,
            request_id,
            surface,
            notebook_id,
            agent_type,
            query_len,
        );
    }

    match state.execute_chat(req).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(e) => {
            let event_name = chat_failure_event_name(&agent_type);
            state
                .record_product_event_if_available(
                    event_name,
                    surface,
                    analytics::ResultTag::Failure,
                    None,
                    notebook_id,
                    serde_json::json!({
                        "agent_type": agent_type,
                        "error_code": e.code(),
                        "query_length": query_len,
                    }),
                )
                .await;
            app_error_response(e)
        }
    }
}

fn accepts_sse(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(',')
                .any(|item| item.trim() == "text/event-stream")
        })
        .unwrap_or(false)
}

fn chat_live_stream_response(
    state: AppState,
    req: ChatRequest,
    request_id: String,
    surface: analytics::Surface,
    notebook_id: Option<Uuid>,
    agent_type: String,
    query_len: usize,
) -> Response {
    let (sender, receiver) = unbounded_channel();
    let request_id_for_task = request_id.clone();
    let agent_type_for_task = agent_type.clone();

    // Shared cancellation token: SseStreamGuard cancels it on stream drop
    // (which happens when the client disconnects), and execute_chat_stream
    // observes it via AgentRequest.cancellation_token to stop work early.
    let cancel = CancellationToken::new();
    let cancel_for_task = cancel.clone();

    tokio::spawn(async move {
        let error_sender = sender.clone();
        if let Err(error) = state
            .execute_chat_stream(req, request_id_for_task.clone(), sender, cancel_for_task)
            .await
        {
            state
                .record_product_event_if_available(
                    chat_failure_event_name(&agent_type_for_task),
                    surface,
                    analytics::ResultTag::Failure,
                    None,
                    notebook_id,
                    serde_json::json!({
                        "agent_type": agent_type_for_task,
                        "error_code": error.code(),
                        "query_length": query_len,
                    }),
                )
                .await;
            let _ = error_sender.send(ChatEvent::Error {
                request_id: request_id_for_task,
                code: error.code().to_string(),
                message: error.message().to_string(),
            });
        }
    });

    sse_response_from_receiver(receiver, surface_label(surface), cancel)
}

fn sse_response_from_receiver(
    mut receiver: UnboundedReceiver<ChatEvent>,
    surface: &'static str,
    cancel: CancellationToken,
) -> Response {
    let stream = async_stream::stream! {
        let _guard = SseStreamGuard(surface, cancel);
        telemetry::prometheus::inc_sse_streams(surface);

        while let Some(event) = receiver.recv().await {
            let event_name = sse_event_name(&event);
            telemetry::prometheus::observe_sse_event(surface, event_name);
            yield Ok::<_, Infallible>(sse_event(event_name, &event));
        }
    };

    let mut response = Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        )
        .into_response();
    add_sse_headers(&mut response);
    response
}

fn sse_event(event_name: &str, payload: &ChatEvent) -> Event {
    Event::default()
        .event(event_name)
        .data(serde_json::to_string(payload).unwrap_or_default())
}

fn sse_event_name(event: &ChatEvent) -> &'static str {
    match event {
        ChatEvent::Start { .. } => "start",
        ChatEvent::Activity { .. } => "activity",
        ChatEvent::AnswerStart { .. } => "answer_start",
        ChatEvent::Trace { .. } => "trace",
        ChatEvent::Token { .. } => "token",
        ChatEvent::ReasoningSummaryDelta { .. } => "reasoning_summary_delta",
        ChatEvent::Citations { .. } => "citations",
        ChatEvent::Done { .. } => "done",
        ChatEvent::Error { .. } => "error",
    }
}

fn chat_failure_event_name(agent_type: &str) -> analytics::ProductEventName {
    if agent_type == "search" {
        analytics::ProductEventName::SearchFailed
    } else {
        analytics::ProductEventName::ChatFailed
    }
}

fn add_sse_headers(response: &mut Response) {
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(
        HeaderName::from_static("x-accel-buffering"),
        HeaderValue::from_static("no"),
    );
}

fn surface_label(surface: analytics::Surface) -> &'static str {
    match surface {
        analytics::Surface::SharedKb => "shared_kb",
        _ => "workspace",
    }
}

struct SseStreamGuard(&'static str, CancellationToken);

impl Drop for SseStreamGuard {
    fn drop(&mut self) {
        telemetry::prometheus::dec_sse_streams(self.0);
        self.1.cancel();
    }
}

// ---------------------------------------------------------------------------
// Search handler
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
pub(crate) struct SearchQueryParams {
    pub q: String,
    #[allow(dead_code)]
    pub scope: Option<String>,
}

pub(crate) async fn search_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Query(params): Query<SearchQueryParams>,
) -> Response {
    let (notebooks, sessions, sources) = state.search(&params.q).await;
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "notebooks": notebooks,
            "sessions": sessions,
            "sources": sources,
        })),
    )
        .into_response()
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct ChatSessionsQuery {
    pub notebook_id: Option<String>,
}

pub(crate) async fn list_chat_sessions_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Query(params): Query<ChatSessionsQuery>,
) -> Response {
    let sessions = state.list_sessions(params.notebook_id.as_deref()).await;
    (
        StatusCode::OK,
        Json(common::ChatSessionListResponse { sessions }),
    )
        .into_response()
}

pub(crate) async fn list_documents_handler(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Response {
    let documents = state.list_documents(None, None).await;
    (
        StatusCode::OK,
        Json(common::DocumentsResponse { documents }),
    )
        .into_response()
}

pub(crate) async fn create_document_upload_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
    Json(req): Json<CreateDocumentRequest>,
) -> Response {
    match state.create_document_upload(&notebook_id, req).await {
        Ok(resp) => (StatusCode::CREATED, Json(resp)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn add_url_source_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
    Json(req): Json<AddUrlSourceRequest>,
) -> Response {
    match state.add_url_source(&notebook_id, req).await {
        Ok(resp) => (StatusCode::CREATED, Json(resp)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn list_sources_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Query(params): Query<ChatSessionsQuery>,
) -> Response {
    let sources = state.list_sources(params.notebook_id.as_deref()).await;
    (StatusCode::OK, Json(common::SourcesResponse { sources })).into_response()
}

pub(crate) async fn update_document_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(document_id): Path<String>,
    Json(req): Json<UpdateDocumentRequest>,
) -> Response {
    match state.update_document(&document_id, req).await {
        Ok(_) => {
            let document = state
                .list_documents(None, Some(&document_id))
                .await
                .into_iter()
                .next();
            match document {
                Some(document) => (StatusCode::OK, Json(document)).into_response(),
                None => error_response(
                    StatusCode::NOT_FOUND,
                    "document_not_found",
                    "document not found",
                ),
            }
        }
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn delete_document_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(document_id): Path<String>,
) -> Response {
    match state.delete_document(&document_id).await {
        Ok(_) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn get_document_status_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(document_id): Path<String>,
) -> Response {
    let document = state
        .list_documents(None, Some(&document_id))
        .await
        .into_iter()
        .next();
    match document {
        Some(document) => (
            StatusCode::OK,
            Json(contracts::documents::DocumentStatusResponse {
                status: document.status.as_str().to_string(),
            }),
        )
            .into_response(),
        None => error_response(
            StatusCode::NOT_FOUND,
            "document_not_found",
            "document not found",
        ),
    }
}

pub(crate) async fn get_document_content_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(document_id): Path<String>,
) -> Response {
    match state.get_document_content(&document_id).await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(error) => app_error_response(error),
    }
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct ParsedPreviewQuery {
    pub cursor: Option<usize>,
    pub limit: Option<usize>,
}

pub(crate) async fn get_parsed_preview_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(document_id): Path<String>,
    Query(params): Query<ParsedPreviewQuery>,
) -> Response {
    match state
        .get_parsed_preview(
            &document_id,
            params.cursor.unwrap_or(0),
            params.limit.unwrap_or(50),
        )
        .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn reindex_document_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(document_id): Path<String>,
) -> Response {
    match state.reindex_document(&document_id).await {
        Ok(_) => (
            StatusCode::ACCEPTED,
            Json(contracts::auth::EmptyResponse {}),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn complete_document_upload_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(document_id): Path<String>,
) -> Response {
    match state.complete_document_upload(&document_id).await {
        Ok(_) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn create_chat_session_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(req): Json<CreateChatSessionRequest>,
) -> Response {
    match state.create_session(req).await {
        Ok(session) => (StatusCode::CREATED, Json(session)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn get_chat_session_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(session_id): Path<String>,
) -> Response {
    match state.get_session(&session_id).await {
        Some(session) => (StatusCode::OK, Json(session)).into_response(),
        None => error_response(
            StatusCode::NOT_FOUND,
            "session_not_found",
            "session not found",
        ),
    }
}

pub(crate) async fn update_chat_session_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(session_id): Path<String>,
    Json(req): Json<UpdateChatSessionRequest>,
) -> Response {
    match state.update_session(&session_id, req).await {
        Ok(session) => (StatusCode::OK, Json(session)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn delete_chat_session_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(session_id): Path<String>,
) -> Response {
    match state.delete_session(&session_id).await {
        Ok(status) => (StatusCode::OK, Json(status)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn get_chat_messages_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(session_id): Path<String>,
) -> Response {
    match state.list_messages(&session_id).await {
        Ok(messages) => (
            StatusCode::OK,
            Json(common::ChatMessageListResponse { messages }),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn citation_lookup_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(req): Json<CitationLookupRequest>,
) -> Response {
    match state
        .lookup_citation(&req.session_id, req.message_id, req.citation_id)
        .await
    {
        Ok(detail) => {
            let metadata = serde_json::json!({
                "message_id": req.message_id,
                "citation_id": req.citation_id,
                "doc_id": detail.doc_id.clone(),
                "chunk_id": detail.chunk_id.clone(),
                "page": detail.page,
            });
            state
                .record_product_event_if_available(
                    analytics::ProductEventName::CitationOpened,
                    analytics::Surface::Workspace,
                    analytics::ResultTag::Success,
                    uuid::Uuid::parse_str(&req.session_id).ok(),
                    None,
                    metadata.clone(),
                )
                .await;
            state
                .record_product_event_if_available(
                    analytics::ProductEventName::SourceFocused,
                    analytics::Surface::Workspace,
                    analytics::ResultTag::Success,
                    uuid::Uuid::parse_str(&req.session_id).ok(),
                    None,
                    metadata,
                )
                .await;
            (StatusCode::OK, Json(detail)).into_response()
        }
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn citation_asset_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(asset_id): Path<String>,
) -> Response {
    match state.get_citation_asset(&asset_id).await {
        Ok((bytes, mime_type)) => {
            (StatusCode::OK, [(header::CONTENT_TYPE, mime_type)], bytes).into_response()
        }
        Err(error) => app_error_response(error),
    }
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct CreateShareRequest {
    pub role: String,
    #[serde(default)]
    pub expires_at: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct UpdateShareSettingsBody {
    #[serde(default)]
    pub access_level: Option<String>,
    #[serde(default)]
    pub allow_download: Option<bool>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct AccessLevelBody {
    pub access_level: String,
}

#[derive(Debug, serde::Serialize)]
struct ApiEnvelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<ApiErrorEnvelope>,
}

#[derive(Debug, serde::Serialize)]
struct ApiErrorEnvelope {
    message: String,
}

pub(crate) async fn create_share_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
    Json(req): Json<CreateShareRequest>,
) -> Response {
    let Some(pg) = state.pg() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };
    let expires_in_secs = req.expires_at.as_deref().and_then(parse_expires_in_secs);
    let access_level = avrag_share::AccessLevel::from_role(&req.role);
    match avrag_share::handle_create_share_link(
        state.auth().clone(),
        notebook_id,
        access_level,
        expires_in_secs,
        pg,
    )
    .await
    {
        Ok(resp) => (StatusCode::OK, resp).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn revoke_share_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((_notebook_id, token)): Path<(String, String)>,
) -> Response {
    let Some(pg) = state.pg() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };
    match avrag_share::handle_revoke_share_link(state.auth().clone(), token, pg).await {
        Ok(()) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn get_share_settings_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
) -> Response {
    let Some(pg) = state.pg() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };
    match avrag_share::handle_get_share_settings(state.auth().clone(), notebook_id, pg).await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn update_share_settings_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
    Json(req): Json<UpdateShareSettingsBody>,
) -> Response {
    let Some(pg) = state.pg() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };
    match avrag_share::handle_update_share_settings(
        state.auth().clone(),
        notebook_id,
        req.access_level,
        req.allow_download,
        pg,
    )
    .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn update_access_level_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
    Json(req): Json<AccessLevelBody>,
) -> Response {
    let Some(pg) = state.pg() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };
    match avrag_share::handle_update_access_level(
        state.auth().clone(),
        notebook_id,
        req.access_level,
        pg,
    )
    .await
    {
        Ok(access_level) => (
            StatusCode::OK,
            Json(serde_json::json!({ "access_level": access_level })),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn get_share_analytics_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
) -> Response {
    let Some(pg) = state.pg() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };
    match avrag_share::handle_get_share_analytics(state.auth().clone(), notebook_id, pg).await {
        Ok(data) => (
            StatusCode::OK,
            Json(ApiEnvelope {
                ok: true,
                data: Some(data),
                error: None,
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<Vec<avrag_share::ShareAnalytics>> {
                ok: false,
                data: None,
                error: Some(ApiErrorEnvelope {
                    message: error.message().to_string(),
                }),
            }),
        )
            .into_response(),
    }
}

pub(crate) async fn get_share_access_logs_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
) -> Response {
    let Some(pg) = state.pg() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };
    match avrag_share::handle_get_share_access_logs(state.auth().clone(), notebook_id, None, pg)
        .await
    {
        Ok(data) => (
            StatusCode::OK,
            Json(ApiEnvelope {
                ok: true,
                data: Some(data),
                error: None,
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<Vec<avrag_share::ShareAccessLog>> {
                ok: false,
                data: None,
                error: Some(ApiErrorEnvelope {
                    message: error.message().to_string(),
                }),
            }),
        )
            .into_response(),
    }
}

pub(crate) async fn validate_share_token_handler(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Response {
    let Some(pg) = state.pg() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };
    match avrag_share::handle_validate_token(&token, pg).await {
        Ok(Some(notebook_id)) => (
            StatusCode::OK,
            Json(ApiEnvelope {
                ok: true,
                data: Some(common::ShareTokenResponse {
                    share_token: notebook_id,
                }),
                error: None,
            }),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::OK,
            Json(ApiEnvelope::<common::ShareTokenResponse> {
                ok: false,
                data: None,
                error: Some(ApiErrorEnvelope {
                    message: "invalid share token".to_string(),
                }),
            }),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn list_api_keys_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
) -> Response {
    match state.list_api_keys(&notebook_id).await {
        Ok(api_keys) => (
            StatusCode::OK,
            Json(common::ApiKeyListResponse { api_keys }),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn create_api_key_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
    Json(req): Json<common::CreateApiKeyRequest>,
) -> Response {
    match state.create_api_key(&notebook_id, req).await {
        Ok(resp) => (StatusCode::CREATED, Json(resp)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn revoke_api_key_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((notebook_id, key_id)): Path<(String, String)>,
) -> Response {
    match state.revoke_api_key(&notebook_id, &key_id).await {
        Ok(_) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct InviteMemberBody {
    pub email: String,
    pub role: String,
}

pub(crate) async fn list_members_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
) -> Response {
    let Some(pg) = state.pg() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };
    match avrag_share::handle_list_members(state.auth().clone(), notebook_id, pg).await {
        Ok(items) => {
            let members = items
                .into_iter()
                .map(|member| contracts::share::MemberRow {
                    member_id: member.id,
                    user_id: member.user_id.unwrap_or_default(),
                    email: member.email.unwrap_or_default(),
                    role: format!("{:?}", member.access_level).to_lowercase(),
                    status: member.invite_status,
                    invited_at: member.invited_at.to_string(),
                })
                .collect();
            (
                StatusCode::OK,
                Json(contracts::share::MembersResponse { members }),
            )
                .into_response()
        }
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn invite_member_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
    Json(req): Json<InviteMemberBody>,
) -> Response {
    let Some(pg) = state.pg() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };
    let role = avrag_share::AccessLevel::from_role(&req.role);
    match avrag_share::handle_invite_member(state.auth().clone(), notebook_id, req.email, role, pg)
        .await
    {
        Ok(_) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn accept_member_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((notebook_id, member_id)): Path<(String, String)>,
) -> Response {
    let Some(pg) = state.pg() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };
    match avrag_share::handle_accept_invite(state.auth().clone(), notebook_id, member_id, pg).await
    {
        Ok(()) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn decline_member_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((notebook_id, member_id)): Path<(String, String)>,
) -> Response {
    let Some(pg) = state.pg() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };
    match avrag_share::handle_decline_invite(state.auth().clone(), notebook_id, member_id, pg).await
    {
        Ok(()) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn remove_member_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((notebook_id, member_id)): Path<(String, String)>,
) -> Response {
    let Some(pg) = state.pg() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };
    match avrag_share::handle_remove_member(state.auth().clone(), notebook_id, member_id, pg).await
    {
        Ok(()) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn list_notifications_handler(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Response {
    match state.list_notifications(100, 0).await {
        Ok(notifications) => (
            StatusCode::OK,
            Json(common::NotificationsResponse { notifications }),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn mark_notification_read_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notification_id): Path<String>,
) -> Response {
    match state.mark_notification_read(&notification_id).await {
        Ok(_) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn message_feedback_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(req): Json<common::MessageFeedbackRequest>,
) -> Response {
    let metadata = serde_json::json!({
        "message_id": req.message_id,
        "rating": req.rating,
    });
    state
        .record_product_event_if_available(
            analytics::ProductEventName::MessageFeedback,
            analytics::Surface::Workspace,
            analytics::ResultTag::Success,
            uuid::Uuid::parse_str(&req.session_id).ok(),
            None,
            metadata,
        )
        .await;
    (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response()
}

fn parse_expires_in_secs(raw: &str) -> Option<i64> {
    let expires_at = chrono::DateTime::parse_from_rfc3339(raw).ok()?;
    let delta = expires_at
        .with_timezone(&chrono::Utc)
        .signed_duration_since(chrono::Utc::now())
        .num_seconds();
    (delta > 0).then_some(delta)
}

// ---------------------------------------------------------------------------
// Agent capabilities
// ---------------------------------------------------------------------------

pub(crate) async fn agent_capabilities_handler() -> Response {
    let response = app::agents::capability::build_capabilities_response();
    (StatusCode::OK, Json(response)).into_response()
}
