//! ChatBubble component - displays a single chat message

use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use leptos::task::spawn;
#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local as spawn;
use std::sync::Arc;
use web_sdk::ApiClient;
use web_sdk::dtos::{AnswerBlock, Citation, CitationLookupRequest};

use crate::api::api_base_url;
use crate::i18n::choose;
use crate::state::auth::use_auth_state;
use crate::state::chat::use_chat_state;
use crate::state::chat::{ChatMessage, ChatRole, RightTab};
use crate::state::ui_prefs::use_ui_prefs_state;
use crate::state::workspace::WorkspaceState;

#[derive(Debug, Clone)]
enum InlineSegment {
    Text(String),
    Citation(u64),
    Image(u64),
}

fn parse_inline_segments(line: &str) -> Vec<InlineSegment> {
    let mut segments = Vec::new();
    let mut remaining = line;

    while let Some(start) = remaining.find("[[") {
        if start > 0 {
            segments.push(InlineSegment::Text(remaining[..start].to_string()));
        }

        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find("]]") else {
            segments.push(InlineSegment::Text(remaining[start..].to_string()));
            return segments;
        };

        let token = after_start[..end].trim();
        if let Ok(id) = token.parse::<u64>() {
            segments.push(InlineSegment::Citation(id));
        } else if let Some(raw_id) = token.strip_prefix("image:") {
            if let Ok(id) = raw_id.trim().parse::<u64>() {
                segments.push(InlineSegment::Image(id));
            } else {
                segments.push(InlineSegment::Text(format!("[[{}]]", token)));
            }
        } else {
            segments.push(InlineSegment::Text(format!("[[{}]]", token)));
        }

        remaining = &after_start[end + 2..];
    }

    if !remaining.is_empty() {
        segments.push(InlineSegment::Text(remaining.to_string()));
    }

    if segments.is_empty() {
        segments.push(InlineSegment::Text(String::new()));
    }

    segments
}

fn citation_by_display_id(citations: &[Citation], display_id: u64) -> Option<Citation> {
    citations.iter().enumerate().find_map(|(index, citation)| {
        let citation_id = if citation.citation_id > 0 {
            citation.citation_id as u64
        } else {
            (index + 1) as u64
        };
        (citation_id == display_id).then(|| citation.clone())
    })
}

fn citation_by_chunk_id(citations: &[Citation], chunk_id: &str) -> Option<(u64, Citation)> {
    citations.iter().enumerate().find_map(|(index, citation)| {
        let citation_chunk_id = citation.chunk_id.as_deref()?;
        if citation_chunk_id != chunk_id {
            return None;
        }
        let display_id = if citation.citation_id > 0 {
            citation.citation_id as u64
        } else {
            (index + 1) as u64
        };
        Some((display_id, citation.clone()))
    })
}

#[cfg(target_arch = "wasm32")]
fn copy_message_to_clipboard(content: &str) {
    if let Some(window) = web_sys::window() {
        let clipboard = window.navigator().clipboard();
        let _ = clipboard.write_text(content);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn copy_message_to_clipboard(_content: &str) {}

/// ChatBubble component displays a single message with appropriate styling
#[component]
pub fn ChatBubble(
    message: ChatMessage,
    on_edit_user: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
    on_regenerate_assistant: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
    on_add_to_note: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
) -> impl IntoView {
    let is_user = message.role == ChatRole::User;
    let auth = use_auth_state();
    let chat_state = use_chat_state();
    let locale = use_ui_prefs_state().locale;
    let workspace = use_context::<WorkspaceState>();
    let citations = message.citations.clone();
    let citations_for_show = citations.clone();
    let citations_for_toggle = citations_for_show.clone();
    let answer_blocks = message.answer_blocks.clone();
    let lines = message
        .content
        .split('\n')
        .map(str::to_string)
        .collect::<Vec<_>>();
    let message_content_for_copy = StoredValue::new(message.content.clone());
    let message_content_for_edit = StoredValue::new(message.content.clone());
    let message_content_for_note = StoredValue::new(message.content.clone());
    let message_id_for_regenerate = StoredValue::new(message.id.clone());

    let select_citation = {
        let auth = auth.clone();
        let chat_state = chat_state.clone();
        let workspace = workspace.clone();
        let message_session_id = message.session_id.clone();
        let message_id = message.server_message_id;
        move |citation: Citation| {
            let mut immediate = citation.clone();
            chat_state.set_active_citation.set(Some(immediate.clone()));
            chat_state.set_active_tab.set(RightTab::Evidence);
            if let Some(workspace) = workspace.clone() {
                workspace.request_citation_focus(&immediate);
            }

            let Some(token) = auth.token.get() else {
                return;
            };
            let Some(session_id) = message_session_id.clone() else {
                return;
            };
            let Some(message_id) = message_id else {
                return;
            };
            let citation_id = citation.citation_id;
            if citation_id <= 0 {
                return;
            }

            let chat_state = chat_state.clone();
            let workspace = workspace.clone();
            spawn(async move {
                let client = ApiClient::new(api_base_url()).with_auth(token);
                let req = CitationLookupRequest {
                    session_id,
                    message_id,
                    citation_id,
                };
                if let Ok(detail) = client.citation_lookup(&req).await {
                    immediate.content = detail.content.or(immediate.content);
                    immediate.page = detail.page.or(immediate.page);
                    immediate.chunk_id = detail.chunk_id.or(immediate.chunk_id);
                    immediate.chunk_type = detail.chunk_type.or(immediate.chunk_type);
                    immediate.asset_id = detail.asset_id.or(immediate.asset_id);
                    immediate.caption = detail.caption.or(immediate.caption);
                    immediate.image_url = detail.image_url.or(immediate.image_url);
                    immediate.doc_name = detail.doc_name.unwrap_or(immediate.doc_name);
                    chat_state.set_active_citation.set(Some(immediate.clone()));
                    if let Some(workspace) = workspace {
                        workspace.request_citation_focus(&immediate);
                    }
                }
            });
        }
    };

    view! {
        <div
            class="flex mb-8 animate-fade-in"
            attr:data-virtual-item-id={message.id.clone()}
            attr:data-virtual-role={message.role.as_str()}
        >
            <div class="flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center mr-4 shadow-sm"
                class=("bg-primary/10", is_user)
                class=("bg-card border border-border", !is_user)
            >
                {if is_user {
                    view! {
                        <svg class="w-4 h-4 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z"/>
                        </svg>
                    }
                } else {
                    view! {
                        <svg class="w-4 h-4 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z"/>
                        </svg>
                    }
                }}
            </div>

            <div class="flex-1 min-w-0">
                <div class="text-xs font-medium mb-1 tracking-wide uppercase"
                    class=("text-primary", is_user)
                    class=("text-muted-foreground", !is_user)
                >
                    {move || {
                        if is_user {
                            choose(locale.get(), "你", "You")
                        } else {
                            choose(locale.get(), "助手", "Assistant")
                        }
                    }}
                </div>

                <div class="prose prose-sm max-w-none text-foreground leading-relaxed"
                    class=("text-foreground/90", is_user)
                    class=("font-medium", is_user)
                    class=("text-foreground", !is_user)
                >
                    <div class="space-y-4">
                        {if !answer_blocks.is_empty() {
                            answer_blocks.into_iter().map(|block| {
                                match block {
                                    AnswerBlock::Text { text, citations: block_citations } => {
                                        let select_citation = select_citation.clone();
                                        view! {
                                            <p class="text-gray-800 whitespace-pre-wrap break-words leading-7">
                                                <span>{text}</span>
                                                {block_citations.into_iter().map(|chunk_id| {
                                                    let mapped = citation_by_chunk_id(&citations, &chunk_id);
                                                    let select_citation = select_citation.clone();
                                                    let button_label = mapped
                                                        .as_ref()
                                                        .map(|(display_id, _)| format!("[{}]", display_id))
                                                        .unwrap_or_default();
                                                    if let Some((_, citation)) = mapped {
                                                        view! {
                                                            <button class="inline-flex items-center justify-center rounded border border-border bg-card/50 px-1.5 py-0.5 text-[10px] font-semibold text-muted-foreground hover:bg-muted hover:text-foreground transition-all duration-200 mx-0.5 align-text-top shadow-sm transform hover:-translate-y-0.5"
                                                                on:click=move |_| {
                                                                    select_citation(citation.clone());
                                                                }
                                                            >
                                                                {button_label.clone()}
                                                            </button>
                                                        }.into_any()
                                                    } else {
                                                        view! { <></> }.into_any()
                                                    }
                                                }).collect_view()}
                                            </p>
                                        }.into_any()
                                    }
                                    AnswerBlock::Image { chunk_id } => {
                                        if let Some((display_id, citation)) = citation_by_chunk_id(&citations, &chunk_id) {
                                            if let Some(image_url) = citation.image_url.clone() {
                                                let caption = citation.caption.clone();
                                                let doc_name = citation.doc_name.clone();
                                                let citation_for_click = citation.clone();
                                                let select_citation = select_citation.clone();
                                                return view! {
                                                    <button
                                                        class="block w-full rounded-xl border border-gray-200 bg-white p-2 text-left hover:bg-gray-50 transition-colors"
                                                        on:click=move |_| {
                                                            select_citation(citation_for_click.clone());
                                                        }
                                                    >
                                                        <img
                                                            src=image_url
                                                            alt=caption.clone().unwrap_or_else(|| doc_name.clone())
                                                            class="max-h-96 w-auto max-w-full rounded-lg object-contain"
                                                        />
                                                        <div class="mt-2 flex items-center justify-between gap-2">
                                                            <div class="text-sm text-gray-700 whitespace-pre-wrap">
                                                                {caption.unwrap_or(doc_name)}
                                                            </div>
                                                            <span class="inline-flex items-center justify-center rounded-full bg-blue-100 px-2 py-0.5 text-xs font-medium text-blue-700">
                                                                {format!("[{}]", display_id)}
                                                            </span>
                                                        </div>
                                                    </button>
                                                }.into_any();
                                            }
                                        }
                                        view! {
                                            <span class="text-xs text-gray-400">{format!("[image:{}]", chunk_id)}</span>
                                        }.into_any()
                                    }
                                }
                            }).collect_view().into_any()
                        } else {
                            lines.into_iter().enumerate().map(|(_line_idx, line)| {
                                let trimmed = line.trim().to_string();
                                if trimmed.is_empty() {
                                    return view! { <div class="h-2"></div> }.into_any();
                                }

                                if let Some(raw_id) = trimmed
                                    .strip_prefix("[[image:")
                                    .and_then(|rest| rest.strip_suffix("]]"))
                                    .map(str::trim)
                                {
                                    if let Ok(display_id) = raw_id.parse::<u64>() {
                                        if let Some(citation) = citation_by_display_id(&citations, display_id) {
                                            if let Some(image_url) = citation.image_url.clone() {
                                                let caption = citation.caption.clone();
                                                let doc_name = citation.doc_name.clone();
                                                let citation_for_click = citation.clone();
                                                let select_citation = select_citation.clone();
                                                return view! {
                                                    <button
                                                        class="block w-full rounded-xl border border-gray-200 bg-white p-2 text-left hover:bg-gray-50 transition-colors"
                                                        on:click=move |_| {
                                                            select_citation(citation_for_click.clone());
                                                        }
                                                    >
                                                        <img
                                                            src=image_url
                                                            alt=caption.clone().unwrap_or_else(|| doc_name.clone())
                                                            class="max-h-96 w-auto max-w-full rounded-lg object-contain"
                                                        />
                                                        <div class="mt-2 flex items-center justify-between gap-2">
                                                            <div class="text-sm text-gray-700 whitespace-pre-wrap">
                                                                {caption.unwrap_or(doc_name)}
                                                            </div>
                                                            <span class="inline-flex items-center justify-center rounded-full bg-blue-100 px-2 py-0.5 text-xs font-medium text-blue-700">
                                                                {format!("[{}]", display_id)}
                                                            </span>
                                                        </div>
                                                    </button>
                                                }.into_any();
                                            }
                                        }
                                    }
                                }

                                let segments = parse_inline_segments(&line);
                                view! {
                                    <p class="text-gray-800 whitespace-pre-wrap break-words leading-7">
                                        {segments.into_iter().enumerate().map(|(_seg_idx, segment)| {
                                            match segment {
                                                InlineSegment::Text(text) => {
                                                    view! { <span>{text}</span> }.into_any()
                                                }
                                                InlineSegment::Citation(display_id) => {
                                                    let citation = citation_by_display_id(&citations, display_id);
                                                    let select_citation = select_citation.clone();
                                                    view! {
                                                        <button class="inline-flex items-center justify-center rounded border border-border bg-card/50 px-1.5 py-0.5 text-[10px] font-semibold text-muted-foreground hover:bg-muted hover:text-foreground transition-all duration-200 mx-0.5 align-text-top shadow-sm transform hover:-translate-y-0.5"
                                                            on:click=move |_| {
                                                                if let Some(citation) = citation.clone() {
                                                                    select_citation(citation);
                                                                }
                                                            }
                                                        >
                                                            {format!("[{}]", display_id)}
                                                        </button>
                                                    }.into_any()
                                                }
                                                InlineSegment::Image(display_id) => {
                                                    view! {
                                                        <span class="text-xs text-gray-400">{format!("[image:{}]", display_id)}</span>
                                                    }.into_any()
                                                }
                                            }
                                        }).collect_view()}
                                    </p>
                                }.into_any()
                            }).collect_view().into_any()
                        }}
                    </div>
                </div>

                <Show when={move || !citations_for_toggle.is_empty()}>
                    <div class="mt-4 flex flex-wrap gap-2 pt-2 border-t border-border">
                        {citations_for_show.iter().enumerate().map(|(idx, citation)| {
                            let citation = citation.clone();
                            let select_citation = select_citation.clone();
                            let display_id = if citation.citation_id > 0 {
                                citation.citation_id as usize
                            } else {
                                idx + 1
                            };
                            view! {
                                <button
                                    class="inline-flex items-center px-2.5 py-1 rounded-md bg-card border border-border text-muted-foreground text-xs font-medium hover:bg-muted hover:text-foreground hover:border-primary/50 transition-all duration-200 shadow-sm"
                                    on:click=move |_| {
                                        select_citation(citation.clone());
                                    }
                                >
                                    <span class="text-primary mr-1.5">{display_id}</span>
                                    <span class="truncate max-w-40">{citation.doc_name.clone()}</span>
                                </button>
                            }
                        }).collect_view()}
                    </div>
                </Show>

                <div class="mt-4 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                    <button
                        type="button"
                        class="rounded-lg px-2 py-1 hover:bg-muted hover:text-foreground"
                        on:click=move |_| {
                            message_content_for_copy
                                .with_value(|content| copy_message_to_clipboard(content))
                        }
                    >
                        {move || choose(locale.get(), "复制", "Copy")}
                    </button>
                    <Show when=move || is_user>
                        <button
                            type="button"
                            class="rounded-lg px-2 py-1 hover:bg-muted hover:text-foreground"
                            on:click=move |_| {
                                let content =
                                    message_content_for_edit.with_value(|content| content.clone());
                                on_edit_user.with_value(|callback| callback(content));
                            }
                        >
                            {move || choose(locale.get(), "编辑", "Edit")}
                        </button>
                    </Show>
                    <Show when=move || !is_user>
                        <button
                            type="button"
                            class="rounded-lg px-2 py-1 hover:bg-muted hover:text-foreground"
                            on:click=move |_| {
                                let content =
                                    message_content_for_note.with_value(|content| content.clone());
                                on_add_to_note.with_value(|callback| callback(content));
                            }
                        >
                            {move || choose(locale.get(), "加入笔记", "Add to Note")}
                        </button>
                        <button
                            type="button"
                            class="rounded-lg px-2 py-1 hover:bg-muted hover:text-foreground"
                            on:click=move |_| {
                                let message_id =
                                    message_id_for_regenerate.with_value(|id| id.clone());
                                on_regenerate_assistant.with_value(|callback| callback(message_id));
                            }
                        >
                            {move || choose(locale.get(), "重新生成", "Regenerate")}
                        </button>
                    </Show>
                </div>
            </div>
        </div>
    }
}
