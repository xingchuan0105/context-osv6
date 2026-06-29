use app_bootstrap::AppState;
use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use common::{AppError, CreateDocumentRequest};
use contracts::notebooks::{CreateNotebookNoteRequest, UpdateNotebookNoteRequest};
use uuid::Uuid;

use crate::RequestState;
use crate::auth_guard::{ensure_user_notebook_access, require_user_session};
use super::super::{app_error_response, error_response};

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

fn notebook_note_from_pref(note: &contracts::preferences::NotebookNotePreference) -> contracts::notebooks::NotebookNote {
    contracts::notebooks::NotebookNote {
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
    preferences: &mut contracts::preferences::UserPreferences,
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
        .push(contracts::preferences::NotebookNotePreference {
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

pub(super) async fn load_notebook_notes(
    state: &AppState,
    notebook_id: &str,
) -> Result<Vec<contracts::notebooks::NotebookNote>, AppError> {
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

async fn require_notebook_notes_access(
    state: &AppState,
    notebook_id: &str,
) -> Result<(), Response> {
    if let Err(error) = ensure_user_notebook_access(state, notebook_id).await {
        return Err(app_error_response(error));
    }
    Ok(())
}

pub(crate) async fn list_notebook_notes_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
) -> Response {
    if let Err(error) = require_user_session(state.auth(), "notebook notes require a signed-in user session") {
        return app_error_response(error);
    }
    if let Err(response) = require_notebook_notes_access(&state, &notebook_id).await {
        return response;
    }
    if state.get_notebook(&notebook_id).await.is_none() {
        return error_response(StatusCode::NOT_FOUND, "not_found", "Notebook not found");
    }

    match load_notebook_notes(&state, &notebook_id).await {
        Ok(notes) => (
            StatusCode::OK,
            Json(contracts::notebooks::NotebookNoteListResponse { notes }),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn get_notebook_note_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((notebook_id, note_id)): Path<(String, String)>,
) -> Response {
    if let Err(error) = require_user_session(state.auth(), "notebook notes require a signed-in user session") {
        return app_error_response(error);
    }
    if let Err(response) = require_notebook_notes_access(&state, &notebook_id).await {
        return response;
    }
    if state.get_notebook(&notebook_id).await.is_none() {
        return error_response(StatusCode::NOT_FOUND, "not_found", "Notebook not found");
    }

    match load_notebook_notes(&state, &notebook_id).await {
        Ok(notes) => match notes.into_iter().find(|note| note.id == note_id) {
            Some(note) => {
                (StatusCode::OK, Json(contracts::notebooks::NotebookNoteResponse { note })).into_response()
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
    if let Err(error) = require_user_session(state.auth(), "notebook notes require a signed-in user session") {
        return app_error_response(error);
    }
    if let Err(response) = require_notebook_notes_access(&state, &notebook_id).await {
        return response;
    }
    if state.get_notebook(&notebook_id).await.is_none() {
        return error_response(StatusCode::NOT_FOUND, "not_found", "Notebook not found");
    }

    let mut preferences = match state.current_user_preferences().await {
        Ok(preferences) => preferences,
        Err(error) => return app_error_response(error),
    };
    let now = chrono::Utc::now().to_rfc3339();
    let note = contracts::preferences::NotebookNotePreference {
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
            Json(contracts::notebooks::NotebookNoteResponse {
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
    if let Err(error) = require_user_session(state.auth(), "notebook notes require a signed-in user session") {
        return app_error_response(error);
    }
    if let Err(response) = require_notebook_notes_access(&state, &notebook_id).await {
        return response;
    }
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
            Json(contracts::notebooks::NotebookNoteResponse { note: response }),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn delete_notebook_note_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((notebook_id, note_id)): Path<(String, String)>,
) -> Response {
    if let Err(error) = require_user_session(state.auth(), "notebook notes require a signed-in user session") {
        return app_error_response(error);
    }
    if let Err(response) = require_notebook_notes_access(&state, &notebook_id).await {
        return response;
    }
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
    if let Err(error) = require_user_session(state.auth(), "notebook notes require a signed-in user session") {
        return app_error_response(error);
    }
    if let Err(response) = require_notebook_notes_access(&state, &notebook_id).await {
        return response;
    }
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
            Json(contracts::notebooks::PromoteNotebookNoteResponse {
                note: response_note,
                source_id: upload.document_id,
            }),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}
