// Dashboard routes - Notebook list and workspace pages

use gloo_timers::future::TimeoutFuture;
use leptos::ev::SubmitEvent;
use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use leptos::task::spawn;
#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local as spawn;
use leptos::task::spawn_local;
use leptos_router::components::A;
use leptos_router::hooks::{use_location, use_navigate, use_params_map, use_query_map};
use leptos_router::NavigateOptions;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use web_sdk::ApiClient;
use web_sdk::dtos::{
    ChatSession, CreateChatSessionRequest, CreateNotebookNoteRequest, CreateNotebookRequest,
    DashboardPreferences, Notebook, NotebookNote, NotebookWorkspacePreference, SourceRow,
    UpdateChatSessionRequest, UpdateNotebookNoteRequest, UpdateNotebookRequest,
    WorkspaceDraftPreference,
};

use crate::api::api_base_url;
use crate::components::chat::{ChatPanel, EvidencePanel};
use crate::components::document::{DocumentDetail, DocumentListItem, DocumentUpload};
use crate::components::{LocaleToggle, NoticeBanner, NoticeTone, StatusBadge};
use crate::i18n::choose;
use crate::load::run_once_after_hydration;
use crate::platform::ui_capabilities;
use crate::state::auth::use_auth_state;
use crate::state::chat::{ChatStatus, provide_chat_state};
use crate::state::ui_prefs::use_ui_prefs_state;
use crate::state::workspace::provide_workspace_state;

fn source_status_terminal(status: &str) -> bool {
    matches!(status, "completed" | "failed" | "ready" | "error")
}

fn source_status_docscope_eligible(status: &str) -> bool {
    matches!(status, "completed" | "ready")
}

#[cfg(target_arch = "wasm32")]
const DASHBOARD_PREFS_STORAGE_KEY: &str = "avrag.dashboard-prefs.v1";

#[cfg(target_arch = "wasm32")]
fn is_mobile() -> bool {
    web_sys::window()
        .and_then(|w| w.inner_width().ok())
        .and_then(|v| v.as_f64())
        .map(|w| w < 768.0)
        .unwrap_or(false)
}

#[cfg(not(target_arch = "wasm32"))]
fn is_mobile() -> bool {
    false
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DashboardPrefs {
    favorite_notebook_ids: Vec<String>,
}

#[cfg(target_arch = "wasm32")]
fn read_dashboard_prefs() -> DashboardPrefs {
    let Some(window) = web_sys::window() else {
        return DashboardPrefs::default();
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return DashboardPrefs::default();
    };
    let Ok(Some(raw)) = storage.get(DASHBOARD_PREFS_STORAGE_KEY) else {
        return DashboardPrefs::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
fn read_dashboard_prefs() -> DashboardPrefs {
    DashboardPrefs::default()
}

#[cfg(target_arch = "wasm32")]
fn write_dashboard_prefs(prefs: &DashboardPrefs) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return;
    };
    if let Ok(raw) = serde_json::to_string(prefs) {
        let _ = storage.set(DASHBOARD_PREFS_STORAGE_KEY, &raw);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn write_dashboard_prefs(_prefs: &DashboardPrefs) {}

fn read_favorite_notebook_ids() -> Vec<String> {
    read_dashboard_prefs().favorite_notebook_ids
}

fn write_favorite_notebook_ids(favorite_notebook_ids: &[String]) {
    let mut prefs = read_dashboard_prefs();
    prefs.favorite_notebook_ids = favorite_notebook_ids.to_vec();
    write_dashboard_prefs(&prefs);
}

fn toggle_favorite_notebook_id(favorite_notebook_ids: &mut Vec<String>, notebook_id: &str) {
    if let Some(index) = favorite_notebook_ids
        .iter()
        .position(|item| item == notebook_id)
    {
        favorite_notebook_ids.remove(index);
    } else {
        favorite_notebook_ids.push(notebook_id.to_string());
    }
}

fn sort_workspace_sessions(sessions: &[ChatSession]) -> Vec<ChatSession> {
    let mut items = sessions.iter().cloned().collect::<Vec<_>>();

    items.sort_by_key(|item| {
        (
            !item.pinned,
            std::cmp::Reverse(item.updated_at.clone()),
            item.id.clone(),
        )
    });
    items
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DraftSyncState {
    Idle,
    Syncing,
    Synced,
    Error,
}

fn workspace_draft_notes(preferences: &DashboardPreferences, notebook_id: &str) -> String {
    preferences
        .workspace_drafts
        .iter()
        .find(|draft| draft.notebook_id == notebook_id)
        .map(|draft| draft.notes.clone())
        .unwrap_or_default()
}

fn upsert_workspace_draft(
    drafts: &mut Vec<WorkspaceDraftPreference>,
    notebook_id: &str,
    notes: String,
) {
    let trimmed = notes.trim().to_string();
    if let Some(index) = drafts
        .iter()
        .position(|draft| draft.notebook_id == notebook_id)
    {
        if trimmed.is_empty() {
            drafts.retain(|draft| draft.notebook_id != notebook_id);
        } else {
            drafts[index].notes = notes;
        }
        return;
    }

    if trimmed.is_empty() {
        return;
    }

    drafts.push(WorkspaceDraftPreference {
        notebook_id: notebook_id.to_string(),
        notes,
    });
}

fn workspace_pinned_source_ids(preferences: &DashboardPreferences, notebook_id: &str) -> Vec<String> {
    preferences
        .workspace_preferences
        .iter()
        .find(|preference| preference.notebook_id == notebook_id)
        .map(|preference| preference.pinned_source_ids.clone())
        .unwrap_or_default()
}

fn upsert_workspace_pinned_sources(
    preferences: &mut Vec<NotebookWorkspacePreference>,
    notebook_id: &str,
    pinned_source_ids: Vec<String>,
) {
    if let Some(existing) = preferences
        .iter_mut()
        .find(|preference| preference.notebook_id == notebook_id)
    {
        existing.pinned_source_ids = pinned_source_ids;
        return;
    }

    preferences.push(NotebookWorkspacePreference {
        notebook_id: notebook_id.to_string(),
        pinned_source_ids,
    });
}

fn sort_workspace_sources(sources: &[SourceRow], pinned_source_ids: &[String]) -> Vec<SourceRow> {
    let mut items = sources.to_vec();
    items.sort_by_key(|item| {
        (
            !pinned_source_ids.iter().any(|id| id == &item.id),
            item.file_name.to_lowercase(),
            item.id.clone(),
        )
    });
    items
}

fn sort_workspace_notes(notes: &[NotebookNote]) -> Vec<NotebookNote> {
    let mut items = notes.to_vec();
    items.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.title.cmp(&right.title))
    });
    items
}

fn sanitize_markdown_filename(title: &str) -> String {
    let mut file_name = title
        .trim()
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            ch if ch.is_control() => '-',
            _ => ch,
        })
        .collect::<String>()
        .trim()
        .to_string();

    if file_name.is_empty() {
        file_name = "note".to_string();
    }

    if !file_name.to_lowercase().ends_with(".md") {
        file_name.push_str(".md");
    }

    file_name
}

fn render_note_markdown(title: &str, content: &str, updated_at: Option<&str>) -> String {
    let normalized_title = title.trim();
    let title = if normalized_title.is_empty() {
        "Untitled note"
    } else {
        normalized_title
    };

    let mut markdown = format!("# {title}\n\n");
    if let Some(updated_at) = updated_at.map(str::trim).filter(|value| !value.is_empty()) {
        markdown.push_str(&format!("_Updated at: {updated_at}_\n\n"));
    }
    markdown.push_str(content.trim_end());
    markdown.push('\n');
    markdown
}

fn upsert_workspace_note(notes: &mut Vec<NotebookNote>, note: NotebookNote) {
    if let Some(existing) = notes.iter_mut().find(|existing| existing.id == note.id) {
        *existing = note;
    } else {
        notes.push(note);
    }
    let sorted = sort_workspace_notes(notes);
    notes.clear();
    notes.extend(sorted);
}

#[cfg(target_arch = "wasm32")]
fn copy_text_to_clipboard(text: &str) {
    if let Some(window) = web_sys::window() {
        let clipboard = window.navigator().clipboard();
        let _ = clipboard.write_text(text);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn copy_text_to_clipboard(_text: &str) {}

#[cfg(target_arch = "wasm32")]
fn prompt_session_title(current: &str) -> Option<String> {
    let window = web_sys::window()?;
    window
        .prompt_with_message_and_default("重命名会话 / Rename session", current)
        .ok()
        .flatten()
}

#[cfg(not(target_arch = "wasm32"))]
fn prompt_session_title(_current: &str) -> Option<String> {
    None
}

fn refresh_workspace_sessions(
    auth_token: ReadSignal<Option<String>>,
    notebook_id: String,
    set_sessions: WriteSignal<Vec<ChatSession>>,
    set_sessions_loading: WriteSignal<bool>,
    show_loading: bool,
) {
    let Some(token) = auth_token.get_untracked() else {
        return;
    };
    if notebook_id.is_empty() {
        return;
    }

    if show_loading {
        set_sessions_loading.set(true);
    }
    let client = ApiClient::new(api_base_url()).with_auth(token);
    spawn(async move {
        if let Ok(resp) = client.list_chat_sessions(Some(&notebook_id)).await {
            set_sessions.set(resp.sessions);
        }
        if show_loading {
            set_sessions_loading.set(false);
        }
    });
}

fn sync_favorite_notebooks_remote(
    auth_token: Option<String>,
    favorite_notebook_ids: Vec<String>,
    locale: crate::i18n::Locale,
    set_error: WriteSignal<String>,
) {
    let Some(token) = auth_token else {
        return;
    };
    let client = ApiClient::new(api_base_url()).with_auth(token);
    spawn(async move {
        match client.get_user_preferences().await {
            Ok(mut preferences) => {
                preferences.dashboard.favorite_notebook_ids = favorite_notebook_ids;
                if let Err(error) = client.update_user_preferences(&preferences).await {
                    set_error.set(format!(
                        "{}: {}",
                        choose(locale, "同步收藏状态失败", "Failed to sync favorites"),
                        error
                    ));
                }
            }
            Err(error) => {
                set_error.set(format!(
                    "{}: {}",
                    choose(
                        locale,
                        "加载账户偏好失败",
                        "Failed to load account preferences"
                    ),
                    error
                ));
            }
        }
    });
}

// ----------------------------------------------------------------------------
// DashboardListPage - Displays notebooks and allows creating new ones
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum ViewMode {
    Card,
    List,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DashboardTab {
    All,
    Mine,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DashboardSort {
    Recent,
    Title,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DashboardCollectionFilter {
    All,
    Favorited,
    Shared,
}

#[cfg(target_arch = "wasm32")]
fn prompt_notebook_title(current: &str) -> Option<String> {
    let window = web_sys::window()?;
    window
        .prompt_with_message_and_default("重命名知识库 / Rename notebook", current)
        .ok()
        .flatten()
}

#[cfg(not(target_arch = "wasm32"))]
fn prompt_notebook_title(_current: &str) -> Option<String> {
    None
}

#[cfg(target_arch = "wasm32")]
fn confirm_notebook_delete(title: &str) -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    window
        .confirm_with_message(&format!(
            "确认删除知识库「{}」吗？该操作不可恢复。\n\nDelete notebook \"{}\"? This cannot be undone.",
            title, title
        ))
        .unwrap_or(false)
}

#[cfg(not(target_arch = "wasm32"))]
fn confirm_notebook_delete(_title: &str) -> bool {
    false
}

#[cfg(target_arch = "wasm32")]
fn encode_query_param(value: &str) -> String {
    js_sys::encode_uri_component(value)
        .as_string()
        .unwrap_or_else(|| value.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn encode_query_param(value: &str) -> String {
    value.to_string()
}
