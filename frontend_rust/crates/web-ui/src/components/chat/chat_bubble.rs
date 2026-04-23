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

stylance::import_style!(
    #[allow(dead_code)]
    workspace_chat_bubble_style,
    "chat_workspace.module.css"
);

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
            class=workspace_chat_bubble_style::message_row
            class=(workspace_chat_bubble_style::message_row_user, is_user)
            attr:data-virtual-item-id={message.id.clone()}
            attr:data-virtual-role={message.role.as_str()}
        >
            <div
                class=workspace_chat_bubble_style::message_column
                class=(workspace_chat_bubble_style::message_column_user, is_user)
                class=(workspace_chat_bubble_style::message_column_assistant, !is_user)
            >
                <div
                    class=workspace_chat_bubble_style::bubble
                    class=(workspace_chat_bubble_style::bubble_user, is_user)
                    class=(workspace_chat_bubble_style::bubble_assistant, !is_user)
                >
                    <div class=workspace_chat_bubble_style::body_stack>
                        {if !answer_blocks.is_empty() {
                            answer_blocks.into_iter().map(|block| {
                                match block {
                                    AnswerBlock::Text { text, citations: block_citations } => {
                                        let select_citation = select_citation.clone();
                                        view! {
                                            <p class=workspace_chat_bubble_style::text_block>
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
                                                            <button class=workspace_chat_bubble_style::inline_citation
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
                                                        class=workspace_chat_bubble_style::image_card
                                                        on:click=move |_| {
                                                            select_citation(citation_for_click.clone());
                                                        }
                                                    >
                                                        <img
                                                            src=image_url
                                                            alt=caption.clone().unwrap_or_else(|| doc_name.clone())
                                                            class=workspace_chat_bubble_style::image_asset
                                                        />
                                                        <div class=workspace_chat_bubble_style::image_meta_row>
                                                            <div class=workspace_chat_bubble_style::image_caption>
                                                                {caption.unwrap_or(doc_name)}
                                                            </div>
                                                            <span class=workspace_chat_bubble_style::image_badge>
                                                                {format!("[{}]", display_id)}
                                                            </span>
                                                        </div>
                                                    </button>
                                                }.into_any();
                                            }
                                        }
                                        view! {
                                            <span class=workspace_chat_bubble_style::image_fallback>{format!("[image:{}]", chunk_id)}</span>
                                        }.into_any()
                                    }
                                }
                            }).collect_view().into_any()
                        } else {
                            lines.into_iter().enumerate().map(|(_line_idx, line)| {
                                let trimmed = line.trim().to_string();
                                if trimmed.is_empty() {
                                    return view! { <div class=workspace_chat_bubble_style::blank_line></div> }.into_any();
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
                                                        class=workspace_chat_bubble_style::image_card
                                                        on:click=move |_| {
                                                            select_citation(citation_for_click.clone());
                                                        }
                                                    >
                                                        <img
                                                            src=image_url
                                                            alt=caption.clone().unwrap_or_else(|| doc_name.clone())
                                                            class=workspace_chat_bubble_style::image_asset
                                                        />
                                                        <div class=workspace_chat_bubble_style::image_meta_row>
                                                            <div class=workspace_chat_bubble_style::image_caption>
                                                                {caption.unwrap_or(doc_name)}
                                                            </div>
                                                            <span class=workspace_chat_bubble_style::image_badge>
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
                                    <p class=workspace_chat_bubble_style::text_block>
                                        {segments.into_iter().enumerate().map(|(_seg_idx, segment)| {
                                            match segment {
                                                InlineSegment::Text(text) => {
                                                    view! { <span>{text}</span> }.into_any()
                                                }
                                                InlineSegment::Citation(display_id) => {
                                                    let citation = citation_by_display_id(&citations, display_id);
                                                    let select_citation = select_citation.clone();
                                                    view! {
                                                        <button class=workspace_chat_bubble_style::inline_citation
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
                                                        <span class=workspace_chat_bubble_style::image_fallback>{format!("[image:{}]", display_id)}</span>
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
                    <div class=workspace_chat_bubble_style::citations_row>
                        <span class=workspace_chat_bubble_style::sources_label>
                            {move || choose(locale.get(), "来源:", "Sources:")}
                        </span>
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
                                    class=workspace_chat_bubble_style::citation_pill
                                    on:click=move |_| {
                                        select_citation(citation.clone());
                                    }
                                >
                                    {display_id}
                                </button>
                            }
                        }).collect_view()}
                    </div>
                </Show>

                <div class=workspace_chat_bubble_style::actions_row>
                    <button
                        type="button"
                        class=workspace_chat_bubble_style::action_button
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
                            class=workspace_chat_bubble_style::action_button
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
                            class=workspace_chat_bubble_style::action_button
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
                            class=workspace_chat_bubble_style::action_button
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
