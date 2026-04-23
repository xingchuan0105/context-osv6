// Dashboard routes - Workspace list and workspace pages

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
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use web_sdk::ApiClient;
#[cfg(test)]
use web_sdk::dtos::WorkspaceDraftPreference;
use web_sdk::dtos::{
    ChatSession, CreateChatSessionRequest, CreateNotebookNoteRequest, CreateNotebookRequest,
    DashboardPreferences, Notebook, NotebookNote, NotebookWorkspacePreference, SourceRow,
    UpdateChatSessionRequest, UpdateNotebookNoteRequest, UpdateNotebookRequest,
};

use crate::api::api_base_url;
use crate::auth_support::logout_current_session;
use crate::components::chat::{ChatPanel, EvidencePanel};
use crate::components::document::{DocumentDetail, DocumentListItem, DocumentUpload};
use crate::components::{ContextOsMark, NoticeBanner, NoticeTone, StatusBadge};
use crate::i18n::choose;
use crate::load::run_once_after_hydration;
use crate::platform::ui_capabilities;
use crate::state::auth::use_auth_state;
use crate::state::chat::{ChatStatus, provide_chat_state};
use crate::state::ui_prefs::use_ui_prefs_state;
use crate::state::workspace::provide_workspace_state;

stylance::import_crate_style!(
    dashboard_style,
    "src/routes/dashboard/dashboard_shell.module.css"
);
stylance::import_crate_style!(
    workspace_style,
    "src/routes/dashboard/workspace_shell.module.css"
);
stylance::import_crate_style!(
    #[allow(dead_code)]
    workspace_ui_style,
    "src/routes/dashboard/workspace_ui.module.css"
);

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
const WORKSPACE_CREATE_COUNTERS_KEY: &str = "avrag.workspace-create-counters.v1";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct WorkspaceCreateCounters {
    counts: HashMap<String, u32>,
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

#[cfg(target_arch = "wasm32")]
fn read_workspace_create_counters() -> WorkspaceCreateCounters {
    let Some(window) = web_sys::window() else {
        return WorkspaceCreateCounters::default();
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return WorkspaceCreateCounters::default();
    };
    let Ok(Some(raw)) = storage.get(WORKSPACE_CREATE_COUNTERS_KEY) else {
        return WorkspaceCreateCounters::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
fn read_workspace_create_counters() -> WorkspaceCreateCounters {
    WorkspaceCreateCounters::default()
}

#[cfg(target_arch = "wasm32")]
fn write_workspace_create_counters(counters: &WorkspaceCreateCounters) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return;
    };
    if let Ok(raw) = serde_json::to_string(counters) {
        let _ = storage.set(WORKSPACE_CREATE_COUNTERS_KEY, &raw);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn write_workspace_create_counters(_counters: &WorkspaceCreateCounters) {}

#[cfg(target_arch = "wasm32")]
fn workspace_today_date_string() -> String {
    let date = js_sys::Date::new_0();
    format!(
        "{:04}-{:02}-{:02}",
        date.get_full_year(),
        date.get_month() + 1,
        date.get_date()
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn workspace_today_date_string() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn workspace_locale_key(locale: crate::i18n::Locale) -> &'static str {
    match locale {
        crate::i18n::Locale::ZhCn => "zh-CN",
        crate::i18n::Locale::En => "en",
    }
}

fn workspace_create_counter_key(locale: crate::i18n::Locale, date: &str) -> String {
    format!("{}:{}", workspace_locale_key(locale), date)
}

fn workspace_default_title(locale: crate::i18n::Locale, date: &str, count: u32) -> String {
    let base = choose(locale, "未命名 Workspace", "Untitled Workspace");
    if count == 0 {
        format!("{base} {date}")
    } else {
        format!("{base} {date}·{}", count + 1)
    }
}

fn workspace_default_title_for_now(locale: crate::i18n::Locale) -> (String, String) {
    let date = workspace_today_date_string();
    let key = workspace_create_counter_key(locale, &date);
    let count = read_workspace_create_counters()
        .counts
        .get(&key)
        .copied()
        .unwrap_or_default();
    (workspace_default_title(locale, &date, count), key)
}

fn bump_workspace_default_title_counter(key: &str) {
    let mut counters = read_workspace_create_counters();
    let entry = counters.counts.entry(key.to_string()).or_default();
    *entry = entry.saturating_add(1);
    write_workspace_create_counters(&counters);
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

#[cfg(test)]
fn workspace_draft_notes(preferences: &DashboardPreferences, notebook_id: &str) -> String {
    preferences
        .workspace_drafts
        .iter()
        .find(|draft| draft.notebook_id == notebook_id)
        .map(|draft| draft.notes.clone())
        .unwrap_or_default()
}

#[cfg(test)]
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

fn workspace_pinned_source_ids(
    preferences: &DashboardPreferences,
    notebook_id: &str,
) -> Vec<String> {
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

fn dashboard_workspace_display_title(notebook: &Notebook) -> String {
    if notebook.title.trim().is_empty() {
        notebook.name.clone()
    } else {
        notebook.title.clone()
    }
}

fn dashboard_notebook_description_label(
    locale: crate::i18n::Locale,
    notebook: &Notebook,
) -> String {
    let description = notebook.description.trim();
    if description.is_empty() {
        choose(locale, "暂无描述", "No description").to_string()
    } else {
        description.to_string()
    }
}

fn dashboard_notebook_date_label(locale: crate::i18n::Locale, iso_string: &str) -> String {
    let date = iso_string.split('T').next().unwrap_or(iso_string);
    let mut parts = date.split('-');
    let (Some(year), Some(month), Some(day)) = (parts.next(), parts.next(), parts.next()) else {
        return iso_string.to_string();
    };

    if locale == crate::i18n::Locale::ZhCn {
        let month = month.trim_start_matches('0');
        let day = day.trim_start_matches('0');
        format!(
            "{year}年{}月{}日",
            if month.is_empty() { "0" } else { month },
            if day.is_empty() { "0" } else { day }
        )
    } else {
        format!("{year}-{month}-{day}")
    }
}

fn dashboard_notebook_status_summary(
    locale: crate::i18n::Locale,
    notebook: &Notebook,
) -> String {
    let status_total = |statuses: &[&str]| -> i64 {
        statuses
            .iter()
            .filter_map(|status| notebook.status_summary.get(*status))
            .copied()
            .sum()
    };

    let ready = status_total(&["ready", "completed"]);
    let processing = status_total(&["pending", "enqueueing", "queued", "processing", "indexing"]);
    let failed = status_total(&["failed", "error"]);
    let mut parts = Vec::new();

    if ready > 0 {
        parts.push(format!("{} {}", ready, choose(locale, "就绪", "ready")));
    }
    if processing > 0 {
        parts.push(format!(
            "{} {}",
            processing,
            choose(locale, "处理中", "processing")
        ));
    }
    if failed > 0 {
        parts.push(format!("{} {}", failed, choose(locale, "异常", "failed")));
    }

    parts.join(" · ")
}

fn dashboard_notebook_role_label(locale: crate::i18n::Locale, is_owner: bool) -> &'static str {
    if is_owner {
        choose(locale, "所有者", "Owner")
    } else {
        choose(locale, "成员", "Member")
    }
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
// DashboardListPage - Displays workspaces and allows creating new ones
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
    Favorites,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DashboardSort {
    Recent,
    Title,
}

#[cfg(target_arch = "wasm32")]
fn prompt_workspace_title(current: &str) -> Option<String> {
    let window = web_sys::window()?;
    window
        .prompt_with_message_and_default("重命名 Workspace / Rename Workspace", current)
        .ok()
        .flatten()
}

#[cfg(not(target_arch = "wasm32"))]
fn prompt_workspace_title(_current: &str) -> Option<String> {
    None
}

#[cfg(target_arch = "wasm32")]
fn confirm_workspace_delete(title: &str) -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    window
        .confirm_with_message(&format!(
            "确认删除 Workspace「{}」吗？该操作不可恢复。\n\nDelete workspace \"{}\"? This cannot be undone.",
            title, title
        ))
        .unwrap_or(false)
}

#[cfg(not(target_arch = "wasm32"))]
fn confirm_workspace_delete(_title: &str) -> bool {
    false
}
