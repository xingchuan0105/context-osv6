use app::AppState;
use axum::{
    Json,
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use common::{
    AppError, CreateDocumentRequest, CreateNotebookNoteRequest, CreateNotebookRequest,
    NotebookListResponse, NotebookResponse, UpdateNotebookNoteRequest, UpdateNotebookRequest,
};
use uuid::Uuid;

use crate::RequestState;
use super::{app_error_response, error_response};
use super::notebook_analysis::NotebookAnalysisCollector;

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

    let collector = NotebookAnalysisCollector;
    let overview = collector.collect_overview(&notebook);
    let sources_summary = collector.collect_sources(&sources, &preferences, &notebook_id);
    let threads = collector.collect_threads(&sessions);
    let notes_summary = collector.collect_notes(&notes);
    let access = collector.collect_access(&state, &notebook_id).await;
    let alerts = collector.build_alerts(&sources_summary, &sessions, &notes, &notes_summary);

    (
        StatusCode::OK,
        Json(common::NotebookAnalysisResponse {
            overview,
            sources: sources_summary,
            threads,
            notes: notes_summary,
            access,
            alerts,
        }),
    )
        .into_response()
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

fn postgres_unavailable_response() -> Response {
    error_response(
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "Database not available",
    )
}

pub(crate) async fn create_share_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
    Json(req): Json<CreateShareRequest>,
) -> Response {
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    let expires_in_secs = req.expires_at.as_deref().and_then(parse_expires_in_secs);
    let access_level = avrag_share::AccessLevel::from_role(&req.role);
    match state
        .create_share_link(notebook_id, access_level, expires_in_secs)
        .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn revoke_share_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((_notebook_id, token)): Path<(String, String)>,
) -> Response {
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    match state.revoke_share_link(token).await {
        Ok(()) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn get_share_settings_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
) -> Response {
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    match state.get_share_settings(notebook_id).await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn update_share_settings_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
    Json(req): Json<UpdateShareSettingsBody>,
) -> Response {
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    match state
        .update_share_settings(notebook_id, req.access_level, req.allow_download)
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
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    match state
        .update_share_access_level(notebook_id, req.access_level)
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
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    match state.get_share_analytics(notebook_id).await {
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
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    match state.get_share_access_logs(notebook_id).await {
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
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    match state.validate_share_token(&token).await {
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
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    match state.list_share_members(notebook_id).await {
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
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    let role = avrag_share::AccessLevel::from_role(&req.role);
    match state
        .invite_share_member(notebook_id, req.email, role)
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
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    match state.accept_share_invite(notebook_id, member_id).await {
        Ok(()) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn decline_member_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((notebook_id, member_id)): Path<(String, String)>,
) -> Response {
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    match state.decline_share_invite(notebook_id, member_id).await {
        Ok(()) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn remove_member_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((notebook_id, member_id)): Path<(String, String)>,
) -> Response {
    if !state.postgres_configured() {
        return postgres_unavailable_response();
    }
    match state.remove_share_member(notebook_id, member_id).await {
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

fn parse_expires_in_secs(raw: &str) -> Option<i64> {
    let expires_at = chrono::DateTime::parse_from_rfc3339(raw).ok()?;
    let delta = expires_at
        .with_timezone(&chrono::Utc)
        .signed_duration_since(chrono::Utc::now())
        .num_seconds();
    (delta > 0).then_some(delta)
}
