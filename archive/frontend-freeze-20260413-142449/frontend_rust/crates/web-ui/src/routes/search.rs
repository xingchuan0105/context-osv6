//! Global search page powered by chat search mode

use std::collections::HashSet;

use leptos::ev::SubmitEvent;
use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use leptos::task::spawn;
#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local as spawn;
use leptos_router::components::A;
use web_sdk::ApiClient;
use web_sdk::dtos::{ChatRequest, ChatSession, Notebook, SourceRef, SourceRow};

use crate::api::api_base_url;
use crate::components::VirtualTextList;
use crate::i18n::{Locale, MessageKey, choose, t};
use crate::load::run_once_after_hydration;
use crate::platform::next_client_id;
use crate::state::auth::use_auth_state;
use crate::state::ui_prefs::use_ui_prefs_state;
use crate::state::virtual_list::{HeightState, compute_window};

const SEARCH_LIST_OVERSCAN: usize = 3;
const SEARCH_VIEWPORT_FALLBACK_PX: f64 = 720.0;

fn next_request_id() -> String {
    next_client_id("search")
}

fn agent_mode_label(locale: Locale, agent_type: &str) -> &'static str {
    match agent_type {
        "search" => t(locale, MessageKey::WsSearch),
        "general" => t(locale, MessageKey::WsGeneral),
        _ => t(locale, MessageKey::WsRag),
    }
}

pub fn search_answer_item_text(answer: &str) -> String {
    answer.to_string()
}

pub fn search_source_preview_text(snippet: Option<&str>, fallback: Option<&str>) -> String {
    snippet.or(fallback).unwrap_or_default().to_string()
}

#[component]
pub fn SearchPage() -> impl IntoView {
    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;
    let (query, set_query) = signal(String::new());
    let (answer, set_answer) = signal(String::new());
    let (sources, set_sources) = signal(Vec::<SourceRef>::new());
    let (all_notebooks, set_all_notebooks) = signal(Vec::<Notebook>::new());
    let (all_sessions, set_all_sessions) = signal(Vec::<ChatSession>::new());
    let (matched_notebooks, set_matched_notebooks) = signal(Vec::<Notebook>::new());
    let (matched_sessions, set_matched_sessions) = signal(Vec::<ChatSession>::new());
    let (matched_sources, set_matched_sources) = signal(Vec::<SourceRow>::new());
    let (catalog_from_backend, set_catalog_from_backend) = signal(false);
    let (selected_notebook, set_selected_notebook) = signal(String::new());
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(String::new());
    let (loaded_token, set_loaded_token) = signal(String::new());
    let (result_scroll_top_px, _set_result_scroll_top_px) = signal(0.0);
    let (result_viewport_height_px, _set_result_viewport_height_px) =
        signal(SEARCH_VIEWPORT_FALLBACK_PX);
    let result_scroller = NodeRef::<leptos::html::Div>::new();

    Effect::new(move |_| {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(container) = result_scroller.get() {
                _set_result_viewport_height_px.set(container.client_height() as f64);
            }
        }
    });

    let auth_for_load = auth.clone();
    run_once_after_hydration(
        move || auth_for_load.token.get().unwrap_or_default(),
        loaded_token,
        set_loaded_token,
        move || {
            let Some(token) = auth.token.get() else {
                return;
            };
            let client = ApiClient::new(api_base_url()).with_auth(token);
            let notebook_client = client.clone();
            spawn(async move {
                if let Ok(resp) = notebook_client.list_notebooks().await {
                    if selected_notebook.get_untracked().is_empty() {
                        if let Some(notebook) = resp.notebooks.first() {
                            set_selected_notebook.set(notebook.id.clone());
                        }
                    }
                    set_all_notebooks.set(resp.notebooks);
                }
            });
            spawn(async move {
                if let Ok(resp) = client.list_chat_sessions(None).await {
                    set_all_sessions.set(resp.sessions);
                }
            });
        },
    );

    let handle_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let Some(token) = auth.token.get() else {
            set_error
                .set(choose(locale.get_untracked(), "尚未登录", "Not authenticated").to_string());
            return;
        };
        let query_value = query.get().trim().to_string();
        if query_value.is_empty() {
            return;
        }

        set_loading.set(true);
        set_error.set(String::new());
        set_answer.set(String::new());
        set_sources.set(Vec::new());
        set_matched_notebooks.set(Vec::new());
        set_matched_sessions.set(Vec::new());
        set_matched_sources.set(Vec::new());
        set_catalog_from_backend.set(false);

        let notebook_id = selected_notebook.get();
        let request_id = next_request_id();

        let client = ApiClient::new(api_base_url()).with_auth(token);
        let search_client = client.clone();
        let query_for_catalog = query_value.clone();

        spawn(async move {
            if let Ok(resp) = search_client.search(&query_for_catalog).await {
                set_matched_notebooks.set(resp.notebooks);
                set_matched_sessions.set(resp.sessions);
                set_matched_sources.set(resp.sources);
                set_catalog_from_backend.set(true);
            }
        });

        spawn(async move {
            let req = ChatRequest {
                query: query_value,
                notebook_id: (!notebook_id.is_empty()).then_some(notebook_id),
                session_id: None,
                agent_type: "search".to_string(),
                source_type: None,
                source_token: None,
                doc_scope: vec![],
                messages: vec![],
                stream: false,
            };

            match client.chat(&req, Some(request_id.as_str())).await {
                Ok(resp) => {
                    set_answer.set(resp.answer);
                    set_sources.set(resp.sources);
                }
                Err(error) => {
                    set_error.set(format!(
                        "{}: {}",
                        t(locale.get_untracked(), MessageKey::SearchFailed),
                        error
                    ));
                }
            }
            set_loading.set(false);
        });
    };

    let local_filtered_notebooks = move || {
        let q = query.get().to_lowercase();
        all_notebooks
            .get()
            .into_iter()
            .filter(|item| {
                let title = item.title.to_lowercase();
                let description = item.description.to_lowercase();
                title.contains(&q) || description.contains(&q)
            })
            .collect::<Vec<_>>()
    };

    let local_filtered_sessions = move || {
        let q = query.get().to_lowercase();
        all_sessions
            .get()
            .into_iter()
            .filter(|item| {
                item.title
                    .clone()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(&q)
            })
            .collect::<Vec<_>>()
    };

    let visible_notebooks = move || {
        if catalog_from_backend.get() {
            matched_notebooks.get()
        } else {
            local_filtered_notebooks()
        }
    };

    let visible_sessions = move || {
        if catalog_from_backend.get() {
            matched_sessions.get()
        } else {
            local_filtered_sessions()
        }
    };

    let visible_sources = move || {
        if catalog_from_backend.get() {
            matched_sources.get()
        } else {
            Vec::new()
        }
    };

    let search_virtual_items = Signal::derive(move || {
        let mut items = Vec::new();
        if !answer.get().is_empty() {
            items.push(HeightState::predicted("search-answer", 240.0));
        }
        for source in sources.get() {
            items.push(HeightState::predicted(
                format!("search-source-{}", source.id),
                128.0,
            ));
        }
        items
    });
    let visible_search_ids = Signal::derive(move || {
        compute_window(
            &search_virtual_items.get(),
            result_scroll_top_px.get(),
            result_viewport_height_px.get(),
            SEARCH_LIST_OVERSCAN,
        )
        .visible_ids
        .into_iter()
        .collect::<HashSet<_>>()
    });

    view! {
        <div class="app-page-shell">
            <div class="mx-auto max-w-5xl">
            <div class="flex items-center justify-between mb-6">
                <div>
                    <h1 class="app-page-title">
                        {move || t(locale.get(), MessageKey::SearchTitle)}
                    </h1>
                    <p class="text-sm text-muted-foreground mt-1">
                        {move || t(locale.get(), MessageKey::SearchDesc)}
                    </p>
                </div>
                <A href="/dashboard" attr:class="app-link">
                    {move || t(locale.get(), MessageKey::SearchBack)}
                </A>
            </div>

            <form on:submit=handle_submit class="app-surface-card p-4 mb-6">
                <div class="flex flex-col gap-3">
                    <Show when=move || !all_notebooks.get().is_empty()>
                        <select
                            class="app-input"
                            on:change=move |ev| set_selected_notebook.set(event_target_value(&ev))
                        >
                            {all_notebooks.get().into_iter().map(|notebook| {
                                let notebook_id = notebook.id.clone();
                                view! {
                                    <option value={notebook_id.clone()} selected={move || selected_notebook.get() == notebook_id}>
                                        {notebook.title}
                                    </option>
                                }
                            }).collect_view()}
                        </select>
                    </Show>
                    <div class="flex gap-3">
                    <input
                        type="text"
                        class="app-input flex-1"
                        placeholder={move || t(locale.get(), MessageKey::SearchPlaceholder)}
                        value=move || query.get()
                        on:input=move |ev| set_query.set(event_target_value(&ev))
                    />
                    <button
                        type="submit"
                        class="app-button-primary"
                        disabled=move || loading.get()
                    >
                        {move || {
                            if loading.get() {
                                t(locale.get(), MessageKey::SearchSearching)
                            } else {
                                t(locale.get(), MessageKey::SearchButton)
                            }
                        }}
                    </button>
                    </div>
                </div>
            </form>

            <Show when=move || !error.get().is_empty()>
                <div class="rounded border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger mb-4">{error.get()}</div>
            </Show>

            <Show when=move || !search_virtual_items.get().is_empty()>
                <div class="app-surface-card p-6 mb-4">
                    <div class="flex items-center justify-between gap-3 mb-3">
                        <h2 class="text-lg font-semibold text-foreground">
                            {move || t(locale.get(), MessageKey::SearchAnswer)}
                        </h2>
                        <div class="flex flex-wrap gap-2 text-xs text-muted-foreground">
                            <span class="rounded-full bg-muted px-2 py-1">
                                {move || format!("{} {}", visible_notebooks().len(), t(locale.get(), MessageKey::SearchNotebooks))}
                            </span>
                            <span class="rounded-full bg-muted px-2 py-1">
                                {move || format!("{} {}", visible_sessions().len(), t(locale.get(), MessageKey::SearchSessions))}
                            </span>
                            <span class="rounded-full bg-muted px-2 py-1">
                                {move || format!("{} {}", sources.get().len(), t(locale.get(), MessageKey::SearchRetrievedSources))}
                            </span>
                        </div>
                    </div>
                    <div
                        class="mt-4 max-h-[70vh] overflow-y-auto pr-1"
                        node_ref=result_scroller
                        data-test-search-scroll
                        on:scroll=move |_ev| {
                            #[cfg(target_arch = "wasm32")]
                            {
                                let container: web_sys::HtmlElement = event_target(&_ev);
                                _set_result_scroll_top_px.set(container.scroll_top() as f64);
                                _set_result_viewport_height_px.set(container.client_height() as f64);
                            }
                        }
                    >
                        <VirtualTextList
                            row_heights=Signal::derive(move || search_virtual_items.get())
                            viewport_height_px=Signal::derive(move || result_viewport_height_px.get())
                            scroll_top_px=Signal::derive(move || result_scroll_top_px.get())
                            overscan=SEARCH_LIST_OVERSCAN
                        >
                            <div class="space-y-4">
                                {move || {
                                    let visible_ids = visible_search_ids.get();
                                    let mut items = Vec::new();

                                    if visible_ids.contains("search-answer") {
                                        let answer_text = search_answer_item_text(&answer.get());
                                        items.push(
                                            view! {
                                                <div class="whitespace-pre-wrap text-foreground">
                                                    {answer_text}
                                                </div>
                                            }
                                            .into_any(),
                                        );
                                    }

                                    items.extend(
                                        sources
                                            .get()
                                            .into_iter()
                                            .filter_map(|source| {
                                                let item_id = format!("search-source-{}", source.id);
                                                if !visible_ids.contains(&item_id) {
                                                    return None;
                                                }

                                                let preview_text =
                                                    search_source_preview_text(source.snippet.as_deref(), None);
                                                let preview_visible = !preview_text.is_empty();

                                                Some(
                                                    view! {
                                                        <div class="rounded border border-border p-3">
                                                            <div class="font-medium text-foreground">{source.title}</div>
                                                            <div class="mt-1 flex flex-wrap gap-2 text-xs text-muted-foreground">
                                                                {if let Some(doc_id) = source.doc_id.as_ref() {
                                                                    view! {
                                                                        <span class="rounded-full bg-muted px-2 py-1">
                                                                            {format!(
                                                                                "{} {}",
                                                                                t(locale.get(), MessageKey::SearchDoc),
                                                                                doc_id.chars().take(8).collect::<String>()
                                                                            )}
                                                                        </span>
                                                                    }.into_any()
                                                                } else {
                                                                    view! { <></> }.into_any()
                                                                }}
                                                                {if let Some(page) = source.page {
                                                                    view! {
                                                                        <span class="rounded-full bg-muted px-2 py-1">
                                                                            {format!("{} {}", t(locale.get(), MessageKey::SearchPage), page)}
                                                                        </span>
                                                                    }.into_any()
                                                                } else {
                                                                    view! { <></> }.into_any()
                                                                }}
                                                            </div>
                                                            <Show when=move || preview_visible>
                                                                <div class="mt-1 text-sm text-muted-foreground">
                                                                    {preview_text.clone()}
                                                                </div>
                                                            </Show>
                                                        </div>
                                                    }
                                                    .into_any(),
                                                )
                                            }),
                                    );

                                    items.into_iter().collect_view()
                                }}
                            </div>
                        </VirtualTextList>
                    </div>
                </div>
            </Show>

            <Show when=move || !visible_sources().is_empty()>
                <div class="app-surface-card p-6 mb-4">
                    <div class="flex items-center justify-between mb-3">
                        <h2 class="text-lg font-semibold text-foreground">
                            {move || t(locale.get(), MessageKey::SearchDocuments)}
                        </h2>
                        <span class="text-xs text-muted-foreground">{move || visible_sources().len()}</span>
                    </div>
                    <div class="space-y-2">
                        {move || visible_sources().into_iter().map(|source| {
                            view! {
                                <A href={format!("/dashboard/{}", source.notebook_id)} attr:class="block rounded border border-border p-3 hover:bg-muted/40">
                                    <div class="font-medium text-foreground">{source.title}</div>
                                    <div class="mt-1 text-xs text-muted-foreground">
                                        {format!("{} | {} | {}", source.notebook_name, source.file_name, source.status)}
                                    </div>
                                </A>
                            }
                        }).collect_view()}
                    </div>
                </div>
            </Show>

            <Show when=move || !visible_notebooks().is_empty()>
                <div class="app-surface-card p-6 mb-4">
                    <div class="flex items-center justify-between mb-3">
                        <h2 class="text-lg font-semibold text-foreground">
                            {move || t(locale.get(), MessageKey::SearchNotebooks)}
                        </h2>
                        <span class="text-xs text-muted-foreground">{move || visible_notebooks().len()}</span>
                    </div>
                    <div class="space-y-2">
                        {move || visible_notebooks().into_iter().map(|notebook| {
                            view! {
                                <A href={format!("/dashboard/{}", notebook.id)} attr:class="block rounded border border-border p-3 hover:bg-muted/40">
                                    <div class="font-medium text-foreground">{notebook.title}</div>
                                    <div class="text-sm text-muted-foreground mt-1">{notebook.description}</div>
                                </A>
                            }
                        }).collect_view()}
                    </div>
                </div>
            </Show>

            <Show when=move || !visible_sessions().is_empty()>
                <div class="app-surface-card p-6 mb-4">
                    <div class="flex items-center justify-between mb-3">
                        <h2 class="text-lg font-semibold text-foreground">
                            {move || t(locale.get(), MessageKey::SearchSessions)}
                        </h2>
                        <span class="text-xs text-muted-foreground">{move || visible_sessions().len()}</span>
                    </div>
                    <div class="space-y-2">
                        {move || visible_sessions().into_iter().map(|session| {
                            view! {
                                <A
                                    href={format!("/dashboard/{}?session={}", session.notebook_id, session.id)}
                                    attr:class="block rounded border border-border p-3 hover:bg-muted/40"
                                >
                                    <div class="font-medium text-foreground">
                                        {session.title.unwrap_or_else(|| t(locale.get(), MessageKey::SearchUntitled).to_string())}
                                    </div>
                                    <div class="text-xs text-muted-foreground mt-1">
                                        {agent_mode_label(locale.get(), &session.agent_type)}
                                    </div>
                                </A>
                            }
                        }).collect_view()}
                    </div>
                </div>
            </Show>
            </div>
        </div>
    }
}
