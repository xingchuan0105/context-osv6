// Shared pages - Share center and public shared notebook access

use std::collections::HashSet;

use futures_util::StreamExt;
use leptos::ev::SubmitEvent;
use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use leptos::task::spawn;
#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local as spawn;
use leptos_router::NavigateOptions;
use leptos_router::components::A;
use leptos_router::hooks::{use_location, use_navigate, use_params_map};
use std::sync::Arc;
use web_sdk::ApiClient;
use web_sdk::dtos::{
    AccessLogsResponse, Citation, MemberRow, ShareAnalyticsResponse, ShareSettings,
    SharedNotebookPayload, SourceRef,
};
use web_sdk::sse::{ChatSseClient, SseEvent};

use crate::api::api_base_url;
use crate::components::UnavailableFeatureCard;
use crate::components::VirtualTextList;
use crate::components::share::{MembersPanel, ShareAccessLogs, ShareAnalytics, ShareSettingsPanel};
use crate::components::{NoticeBanner, NoticeTone};
use crate::i18n::{MessageKey, choose, t};
use crate::load::run_once_after_hydration;
use crate::platform::{next_client_id, ui_capabilities};
use crate::state::auth::use_auth_state;
use crate::state::ui_prefs::use_ui_prefs_state;
use crate::state::virtual_list::{HeightState, compute_window};

stylance::import_crate_style!(
    #[allow(dead_code)]
    shared_page_style,
    "src/routes/shared/shared_pages.module.css"
);

const SHARED_LIST_OVERSCAN: usize = 3;
const SHARED_VIEWPORT_FALLBACK_PX: f64 = 720.0;

fn next_request_id() -> String {
    next_client_id("shared")
}

fn permission_label(locale: crate::i18n::Locale, permission: &str) -> String {
    match permission {
        "private" => choose(locale, "私有", "private").to_string(),
        "link" => choose(locale, "仅链接", "link").to_string(),
        "public" => choose(locale, "公开", "public").to_string(),
        "viewer" => choose(locale, "查看者", "viewer").to_string(),
        "editor" => choose(locale, "编辑者", "editor").to_string(),
        _ => permission.to_string(),
    }
}

fn source_status_label(locale: crate::i18n::Locale, status: &str) -> String {
    match status {
        "completed" | "ready" => choose(locale, "可用", "Ready").to_string(),
        "pending" | "enqueueing" | "queued" => choose(locale, "排队中", "Queued").to_string(),
        "processing" => choose(locale, "处理中", "Processing").to_string(),
        "failed" | "error" => choose(locale, "失败", "Failed").to_string(),
        _ => status.to_string(),
    }
}

pub fn shared_answer_item_text(streaming_answer: &str, final_answer: &str) -> String {
    if !streaming_answer.is_empty() {
        streaming_answer.to_string()
    } else {
        final_answer.to_string()
    }
}

pub fn shared_source_preview_text(preview: Option<&str>, fallback: Option<&str>) -> String {
    preview.or(fallback).unwrap_or_default().to_string()
}

pub fn typed_citations_from_values(values: Vec<serde_json::Value>) -> Vec<Citation> {
    values
        .into_iter()
        .filter_map(|value| serde_json::from_value::<Citation>(value).ok())
        .collect()
}

pub fn shared_chat_sources_from_citations(citations: &[Citation]) -> Vec<SourceRef> {
    let mut seen_ids = HashSet::new();

    citations
        .iter()
        .filter_map(|citation| {
            let source_id = citation
                .chunk_id
                .clone()
                .or_else(|| citation.asset_id.clone())
                .unwrap_or_else(|| format!("citation-{}", citation.citation_id));

            if !seen_ids.insert(source_id.clone()) {
                return None;
            }

            Some(SourceRef {
                id: source_id,
                title: citation.doc_name.clone(),
                snippet: citation
                    .preview
                    .clone()
                    .or_else(|| citation.content.clone()),
                doc_id: Some(citation.doc_id.clone()),
                page: citation.page,
            })
        })
        .collect()
}

fn payload_answer(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("answer")
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn payload_degrade_reasons(payload: &serde_json::Value) -> Vec<String> {
    payload
        .get("degrade_trace")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get("reason")
                        .and_then(|value| value.as_str())
                        .map(str::to_string)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

/// Tab types for ShareCenterPage
#[derive(Clone, Copy, PartialEq)]
enum ShareTab {
    Settings,
    Analytics,
    AccessLogs,
}

fn share_tab_from_path(pathname: &str) -> ShareTab {
    if pathname.ends_with("/share/analytics") {
        ShareTab::Analytics
    } else if pathname.ends_with("/share/access-logs") {
        ShareTab::AccessLogs
    } else {
        ShareTab::Settings
    }
}

fn share_base_href_from_path(pathname: &str, notebook_id: &str) -> String {
    if pathname.starts_with("/preview/live/workspace/") {
        format!("/preview/live/workspace/{notebook_id}/share")
    } else if pathname.starts_with("/notebooks/") {
        format!("/notebooks/{notebook_id}/share")
    } else {
        format!("/dashboard/{notebook_id}/share")
    }
}

fn share_tab_href(base_href: &str, tab: ShareTab) -> String {
    match tab {
        ShareTab::Settings => base_href.to_string(),
        ShareTab::Analytics => format!("{base_href}/analytics"),
        ShareTab::AccessLogs => format!("{base_href}/access-logs"),
    }
}

// ----------------------------------------------------------------------------
// ShareCenterPage - Manage sharing settings for a notebook
// ----------------------------------------------------------------------------
