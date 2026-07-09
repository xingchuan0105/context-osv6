use app_bootstrap::AppState;
use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use common::{AppError, CreateDocumentRequest};
use contracts::workspaces::{CreateWorkspaceNoteRequest, UpdateWorkspaceNoteRequest};
use uuid::Uuid;

use super::super::{app_error_response, error_response};
use crate::middleware::RequestState;
use crate::auth_guard::{ensure_user_workspace_access, require_user_session};

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

fn notebook_note_from_pref(
    note: &contracts::preferences::WorkspaceNotePreference,
) -> contracts::workspaces::WorkspaceNote {
    contracts::workspaces::WorkspaceNote {
        id: note.note_id.clone(),
        workspace_id: note.workspace_id.clone(),
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
    workspace_id: &str,
) -> bool {
    let has_notes = preferences
        .dashboard
        .workspace_notes
        .iter()
        .any(|note| note.workspace_id == workspace_id);
    if has_notes {
        return false;
    }

    let Some(index) = preferences
        .dashboard
        .workspace_drafts
        .iter()
        .position(|draft| draft.workspace_id == workspace_id && !draft.notes.trim().is_empty())
    else {
        return false;
    };

    let legacy = preferences.dashboard.workspace_drafts.remove(index);
    let now = chrono::Utc::now().to_rfc3339();
    preferences
        .dashboard
        .workspace_notes
        .push(contracts::preferences::WorkspaceNotePreference {
            note_id: Uuid::new_v4().to_string(),
            workspace_id: workspace_id.to_string(),
            title: "Imported Notes".to_string(),
            content: legacy.notes,
            created_at: now.clone(),
            updated_at: now,
            promoted_document_id: None,
            promoted_at: None,
        });
    true
}

pub(super) async fn load_workspace_notes(
    state: &AppState,
    workspace_id: &str,
) -> Result<Vec<contracts::workspaces::WorkspaceNote>, AppError> {
    let mut preferences = state.prefs().current().await?;
    let migrated = migrate_workspace_draft_to_note(&mut preferences, workspace_id);
    if migrated {
        state.prefs().save_current(&preferences).await?;
    }

    let mut notes = preferences
        .dashboard
        .workspace_notes
        .iter()
        .filter(|note| note.workspace_id == workspace_id)
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

async fn require_workspace_notes_access(
    state: &AppState,
    workspace_id: &str,
) -> Result<(), Response> {
    if let Err(error) = ensure_user_workspace_access(state, workspace_id).await {
        return Err(app_error_response(error));
    }
    Ok(())
}

pub(crate) async fn list_workspace_notes_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(workspace_id): Path<String>,
) -> Response {
    if let Err(error) = require_user_session(
        state.auth(),
        "notebook notes require a signed-in user session",
    ) {
        return app_error_response(error);
    }
    if let Err(response) = require_workspace_notes_access(&state, &workspace_id).await {
        return response;
    }
    if state.docs().get_workspace(&workspace_id).await.is_none() {
        return error_response(StatusCode::NOT_FOUND, "not_found", "Workspace not found");
    }

    match load_workspace_notes(&state, &workspace_id).await {
        Ok(notes) => (
            StatusCode::OK,
            Json(contracts::workspaces::WorkspaceNoteListResponse { notes }),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn get_workspace_note_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((workspace_id, note_id)): Path<(String, String)>,
) -> Response {
    if let Err(error) = require_user_session(
        state.auth(),
        "notebook notes require a signed-in user session",
    ) {
        return app_error_response(error);
    }
    if let Err(response) = require_workspace_notes_access(&state, &workspace_id).await {
        return response;
    }
    if state.docs().get_workspace(&workspace_id).await.is_none() {
        return error_response(StatusCode::NOT_FOUND, "not_found", "Workspace not found");
    }

    match load_workspace_notes(&state, &workspace_id).await {
        Ok(notes) => match notes.into_iter().find(|note| note.id == note_id) {
            Some(note) => (
                StatusCode::OK,
                Json(contracts::workspaces::WorkspaceNoteResponse { note }),
            )
                .into_response(),
            None => error_response(StatusCode::NOT_FOUND, "not_found", "Note not found"),
        },
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn create_workspace_note_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(workspace_id): Path<String>,
    Json(req): Json<CreateWorkspaceNoteRequest>,
) -> Response {
    if let Err(error) = require_user_session(
        state.auth(),
        "notebook notes require a signed-in user session",
    ) {
        return app_error_response(error);
    }
    if let Err(response) = require_workspace_notes_access(&state, &workspace_id).await {
        return response;
    }
    if state.docs().get_workspace(&workspace_id).await.is_none() {
        return error_response(StatusCode::NOT_FOUND, "not_found", "Workspace not found");
    }

    let mut preferences = match state.prefs().current().await {
        Ok(preferences) => preferences,
        Err(error) => return app_error_response(error),
    };
    let now = chrono::Utc::now().to_rfc3339();
    let note = contracts::preferences::WorkspaceNotePreference {
        note_id: Uuid::new_v4().to_string(),
        workspace_id: workspace_id.clone(),
        title: normalize_note_title(req.title),
        content: req.content.unwrap_or_default(),
        created_at: now.clone(),
        updated_at: now,
        promoted_document_id: None,
        promoted_at: None,
    };
    preferences.dashboard.workspace_notes.push(note.clone());

    match state.prefs().save_current(&preferences).await {
        Ok(_) => (
            StatusCode::CREATED,
            Json(contracts::workspaces::WorkspaceNoteResponse {
                note: notebook_note_from_pref(&note),
            }),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn update_workspace_note_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((workspace_id, note_id)): Path<(String, String)>,
    Json(req): Json<UpdateWorkspaceNoteRequest>,
) -> Response {
    if let Err(error) = require_user_session(
        state.auth(),
        "notebook notes require a signed-in user session",
    ) {
        return app_error_response(error);
    }
    if let Err(response) = require_workspace_notes_access(&state, &workspace_id).await {
        return response;
    }
    if state.docs().get_workspace(&workspace_id).await.is_none() {
        return error_response(StatusCode::NOT_FOUND, "not_found", "Workspace not found");
    }

    let mut preferences = match state.prefs().current().await {
        Ok(preferences) => preferences,
        Err(error) => return app_error_response(error),
    };
    let migrated = migrate_workspace_draft_to_note(&mut preferences, &workspace_id);
    let Some(note) = preferences
        .dashboard
        .workspace_notes
        .iter_mut()
        .find(|note| note.workspace_id == workspace_id && note.note_id == note_id)
    else {
        if migrated {
            let _ = state.prefs().save_current(&preferences).await;
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

    match state.prefs().save_current(&preferences).await {
        Ok(_) => (
            StatusCode::OK,
            Json(contracts::workspaces::WorkspaceNoteResponse { note: response }),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn delete_workspace_note_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((workspace_id, note_id)): Path<(String, String)>,
) -> Response {
    if let Err(error) = require_user_session(
        state.auth(),
        "notebook notes require a signed-in user session",
    ) {
        return app_error_response(error);
    }
    if let Err(response) = require_workspace_notes_access(&state, &workspace_id).await {
        return response;
    }
    if state.docs().get_workspace(&workspace_id).await.is_none() {
        return error_response(StatusCode::NOT_FOUND, "not_found", "Workspace not found");
    }

    let mut preferences = match state.prefs().current().await {
        Ok(preferences) => preferences,
        Err(error) => return app_error_response(error),
    };
    let before = preferences.dashboard.workspace_notes.len();
    preferences
        .dashboard
        .workspace_notes
        .retain(|note| !(note.workspace_id == workspace_id && note.note_id == note_id));
    if before == preferences.dashboard.workspace_notes.len() {
        return error_response(StatusCode::NOT_FOUND, "not_found", "Note not found");
    }

    match state.prefs().save_current(&preferences).await {
        Ok(_) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn promote_notebook_note_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((workspace_id, note_id)): Path<(String, String)>,
) -> Response {
    if let Err(error) = require_user_session(
        state.auth(),
        "notebook notes require a signed-in user session",
    ) {
        return app_error_response(error);
    }
    if let Err(response) = require_workspace_notes_access(&state, &workspace_id).await {
        return response;
    }
    if state.docs().get_workspace(&workspace_id).await.is_none() {
        return error_response(StatusCode::NOT_FOUND, "not_found", "Workspace not found");
    }

    let mut preferences = match state.prefs().current().await {
        Ok(preferences) => preferences,
        Err(error) => return app_error_response(error),
    };
    let migrated = migrate_workspace_draft_to_note(&mut preferences, &workspace_id);
    let Some(note) = preferences
        .dashboard
        .workspace_notes
        .iter_mut()
        .find(|note| note.workspace_id == workspace_id && note.note_id == note_id)
    else {
        if migrated {
            let _ = state.prefs().save_current(&preferences).await;
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
    let upload = match state.docs()
        .create_document_upload(
            &workspace_id,
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

    if let Err(error) = state.docs()
        .put_uploaded_document(&upload.document_id, markdown.into_bytes())
        .await
    {
        return app_error_response(error);
    }
    if let Err(error) = state.docs().complete_document_upload(&upload.document_id).await {
        return app_error_response(error);
    }

    let promoted_at = chrono::Utc::now().to_rfc3339();
    note.promoted_document_id = Some(upload.document_id.clone());
    note.promoted_at = Some(promoted_at);
    note.updated_at = chrono::Utc::now().to_rfc3339();
    let response_note = notebook_note_from_pref(note);

    match state.prefs().save_current(&preferences).await {
        Ok(_) => (
            StatusCode::OK,
            Json(contracts::workspaces::PromoteWorkspaceNoteResponse {
                note: response_note,
                source_id: upload.document_id,
            }),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}
