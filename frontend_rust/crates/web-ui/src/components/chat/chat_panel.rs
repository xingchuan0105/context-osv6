//! ChatPanel component - main chat interface with SSE streaming

use futures_util::StreamExt;
use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use leptos::task::spawn;
#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local as spawn;
use std::sync::Arc;
use web_sdk::dtos::AnswerBlock;
use web_sdk::sse::{ChatSseClient, SseEvent};

use super::chat_bubble::ChatBubble;
use super::virtual_items::{ChatVirtualItem, chat_message_to_virtual_item};
use crate::api::api_base_url;
use crate::components::VirtualTextList;
use crate::i18n::{Locale, choose};
use crate::platform::next_client_id;
use crate::routes::shared::{shared_chat_sources_from_citations, typed_citations_from_values};
use crate::state::auth::use_auth_state;
use crate::state::chat::use_chat_state;
use crate::state::chat::{AgentMode, ChatRole, ChatStatus};
use crate::state::ui_prefs::use_ui_prefs_state;
use crate::state::virtual_list::{HeightState, compute_window};
use crate::state::workspace::use_workspace_state;

stylance::import_style!(
    #[allow(dead_code)]
    workspace_chat_style,
    "chat_workspace.module.css"
);

const CHAT_LIST_OVERSCAN: usize = 4;
const CHAT_VIEWPORT_FALLBACK_PX: f64 = 720.0;

#[cfg(target_arch = "wasm32")]
const CHAT_AGENT_MODE_KEY_PREFIX: &str = "avrag.workspace-chat-agent-mode.v2";
#[cfg(target_arch = "wasm32")]
const CHAT_AGENT_MODE_LEGACY_KEY: &str = "avrag.workspace-chat-agent-mode.v1";

#[cfg(target_arch = "wasm32")]
fn read_saved_agent_mode(notebook_id: &str) -> Option<AgentMode> {
    let window = web_sys::window()?;
    let storage = window.local_storage().ok().flatten()?;
    let storage_key = if notebook_id.trim().is_empty() {
        CHAT_AGENT_MODE_LEGACY_KEY.to_string()
    } else {
        format!("{CHAT_AGENT_MODE_KEY_PREFIX}:{notebook_id}")
    };
    let raw = storage
        .get(&storage_key)
        .ok()
        .flatten()
        .or_else(|| storage.get(CHAT_AGENT_MODE_LEGACY_KEY).ok().flatten())?;
    match raw.as_str() {
        "rag" => Some(AgentMode::Rag),
        "search" => Some(AgentMode::Search),
        "general" => Some(AgentMode::General),
        _ => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn read_saved_agent_mode(_notebook_id: &str) -> Option<AgentMode> {
    None
}

#[cfg(target_arch = "wasm32")]
fn write_saved_agent_mode(notebook_id: &str, mode: AgentMode) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return;
    };
    let storage_key = if notebook_id.trim().is_empty() {
        CHAT_AGENT_MODE_LEGACY_KEY.to_string()
    } else {
        format!("{CHAT_AGENT_MODE_KEY_PREFIX}:{notebook_id}")
    };
    let _ = storage.set(&storage_key, mode.as_str());
}

#[cfg(not(target_arch = "wasm32"))]
fn write_saved_agent_mode(_notebook_id: &str, _mode: AgentMode) {}

fn next_request_id() -> String {
    next_client_id("chat")
}

fn agent_mode_label(locale: Locale, mode: AgentMode) -> &'static str {
    match mode {
        AgentMode::Rag => choose(locale, "知识库", "RAG"),
        AgentMode::Search => choose(locale, "网页", "Web"),
        AgentMode::General => choose(locale, "聊天", "Chat"),
    }
}

fn status_label(locale: Locale, status: ChatStatus) -> &'static str {
    match status {
        ChatStatus::Idle => choose(locale, "就绪", "Ready"),
        ChatStatus::Submitting => choose(locale, "发送中...", "Sending..."),
        ChatStatus::Streaming => choose(locale, "生成中...", "Receiving..."),
        ChatStatus::Done => choose(locale, "已完成", "Complete"),
        ChatStatus::Error => choose(locale, "发生错误", "Error occurred"),
    }
}

fn trace_detail_summary(detail: &Option<serde_json::Value>) -> String {
    match detail {
        Some(serde_json::Value::String(message)) => message.clone(),
        Some(serde_json::Value::Object(map)) => map
            .get("message")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| {
                serde_json::to_string_pretty(&serde_json::Value::Object(map.clone()))
                    .unwrap_or_default()
            }),
        Some(value) => value.to_string(),
        None => String::new(),
    }
}

fn payload_answer_blocks(payload: &serde_json::Value) -> Option<Vec<AnswerBlock>> {
    payload
        .get("answer_blocks")
        .cloned()
        .and_then(|value| serde_json::from_value::<Vec<AnswerBlock>>(value).ok())
}

fn payload_citations(payload: &serde_json::Value) -> Option<Vec<web_sdk::dtos::Citation>> {
    payload
        .get("citations")
        .and_then(|value| value.as_array())
        .map(|items| typed_citations_from_values(items.clone()))
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

#[component]
pub fn ChatPanel(
    notebook_id: String,
    append_to_note: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
) -> impl IntoView {
    let chat = use_chat_state();
    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;
    let workspace = use_workspace_state();
    let notebook_id_for_submit = notebook_id.clone();
    let notebook_id_for_bootstrap = StoredValue::new(notebook_id.clone());
    let notebook_id_for_mode_write = StoredValue::new(notebook_id.clone());

    let (input_text, set_input_text) = signal(String::new());
    let (is_submitting, set_is_submitting) = signal(false);
    let (show_mode_menu, set_show_mode_menu) = signal(false);
    let (agent_mode_bootstrapped_for_workspace, set_agent_mode_bootstrapped_for_workspace) =
        signal(String::new());
    let (scroll_top_px, _set_scroll_top_px) = signal(0.0);
    #[allow(unused_variables)]
    let (viewport_height_px, set_viewport_height_px) = signal(CHAT_VIEWPORT_FALLBACK_PX);

    Effect::new(move |_| {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window()
                && let Ok(height) = window.inner_height()
                && let Some(height) = height.as_f64()
            {
                set_viewport_height_px.set(height);
            }
        }
    });

    Effect::new(move |_| {
        let current_workspace_id = notebook_id_for_bootstrap.with_value(|value| value.clone());
        if agent_mode_bootstrapped_for_workspace.get() == current_workspace_id {
            return;
        }
        if let Some(saved_mode) = read_saved_agent_mode(&current_workspace_id)
            && saved_mode != chat.agent_mode.get_untracked()
        {
            chat.set_agent_mode.set(saved_mode);
        }
        set_agent_mode_bootstrapped_for_workspace.set(current_workspace_id);
    });

    let virtual_items = Signal::derive(move || {
        let messages = chat.messages.get();
        let tail_message_id = if chat.status.get() == ChatStatus::Streaming {
            messages.last().and_then(|message| {
                (message.role == ChatRole::Assistant).then(|| message.id.clone())
            })
        } else {
            None
        };

        messages
            .iter()
            .map(|message| {
                let pinned_tail = tail_message_id
                    .as_ref()
                    .map(|tail_id| tail_id == &message.id)
                    .unwrap_or(false);
                chat_message_to_virtual_item(message, pinned_tail)
            })
            .collect::<Vec<ChatVirtualItem>>()
    });
    let row_heights = Signal::derive(move || {
        virtual_items
            .get()
            .into_iter()
            .map(|item| HeightState::predicted(item.id.clone(), item.predicted_height_px()))
            .collect::<Vec<_>>()
    });
    let visible_ids = Signal::derive(move || {
        let items = virtual_items.get();
        let tail_id = items
            .iter()
            .find(|item| item.pinned_tail)
            .map(|item| item.id.clone());
        let window = compute_window(
            &row_heights.get(),
            scroll_top_px.get(),
            viewport_height_px.get(),
            CHAT_LIST_OVERSCAN,
        );
        window
            .pin_tail(tail_id.as_deref().unwrap_or(""))
            .visible_ids
    });

    let submit_query: Arc<dyn Fn(String) + Send + Sync> = Arc::new({
        let chat = chat.clone();
        let auth = auth.clone();
        let workspace = workspace.clone();
        move |query: String| {
            let query = query.trim().to_string();
            if query.is_empty() || is_submitting.get_untracked() {
                return;
            }

            let token = match auth.token.get() {
                Some(token) => token,
                None => {
                    chat.set_error(
                        choose(locale.get_untracked(), "尚未登录", "Not authenticated").to_string(),
                    );
                    return;
                }
            };

            let selected_doc_scope = if chat.agent_mode.get() == AgentMode::Rag {
                workspace
                    .sources
                    .get()
                    .into_iter()
                    .filter(|source| {
                        workspace
                            .selected_source_ids
                            .get()
                            .iter()
                            .any(|id| id == &source.id)
                            && matches!(source.status.as_str(), "completed" | "ready")
                    })
                    .map(|source| source.id)
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };

            if chat.agent_mode.get() == AgentMode::Rag && selected_doc_scope.is_empty() {
                chat.set_error(
                    choose(
                        locale.get_untracked(),
                        "使用知识库模式前，请先在资料区选择至少一份已完成索引的资料。",
                        "Please select at least one completed source before using RAG.",
                    )
                    .to_string(),
                );
                return;
            }

            set_input_text.set(String::new());
            set_is_submitting.set(true);
            chat.start_submit(query.clone());

            let client = ChatSseClient::new(api_base_url()).with_auth(token);
            let notebook_id_clone = notebook_id_for_submit.clone();
            let agent_mode = chat.agent_mode.get();
            let chat_for_async = chat.clone();
            let active_session_id = chat.session_id.get();
            let request_id = next_request_id();

            spawn(async move {
                match client
                    .stream_chat_with_request(
                        web_sdk::dtos::ChatRequest {
                            query: query.clone(),
                            notebook_id: Some(notebook_id_clone.clone()),
                            session_id: active_session_id.clone(),
                            agent_type: agent_mode.as_str().to_string(),
                            source_type: None,
                            source_token: None,
                            doc_scope: selected_doc_scope.clone(),
                            messages: vec![],
                            stream: true,
                        },
                        Some(request_id.as_str()),
                    )
                    .await
                {
                    Ok(stream) => {
                        let mut stream = stream;
                        let mut current_content = String::new();
                        let mut current_citations = Vec::new();

                        while let Some(event) = stream.next().await {
                            match event {
                                SseEvent::Start { session_id, .. } => {
                                    if !session_id.is_empty() {
                                        chat_for_async.set_session(session_id.clone());
                                        chat_for_async.push_trace(format!("start: {}", session_id));
                                    }
                                }
                                SseEvent::Trace {
                                    stage,
                                    status,
                                    detail,
                                    ..
                                } => {
                                    let message = trace_detail_summary(&detail);
                                    let trace_entry = if message.is_empty() {
                                        format!("{stage} [{status}]")
                                    } else {
                                        format!("{stage} [{status}] {message}")
                                    };
                                    chat_for_async.push_trace(trace_entry);

                                    if let Some(mode) = detail
                                        .as_ref()
                                        .and_then(|value| value.get("mode"))
                                        .and_then(|value| value.as_str())
                                    {
                                        chat_for_async.set_planner_mode_value(mode.to_string());
                                    }

                                    if let Some(trace_json) = detail.clone().filter(|value| {
                                        stage.contains("rag")
                                            || stage.contains("retriev")
                                            || value.get("item_count").is_some()
                                            || value.get("source_ids").is_some()
                                    }) {
                                        if let Some(source_count) = trace_json
                                            .get("top_k_returned")
                                            .and_then(|value| value.as_u64())
                                            .or_else(|| {
                                                trace_json
                                                    .get("source_count")
                                                    .and_then(|value| value.as_u64())
                                            })
                                            .or_else(|| {
                                                trace_json
                                                    .get("source_ids")
                                                    .and_then(|value| value.as_array())
                                                    .map(|items| items.len() as u64)
                                            })
                                        {
                                            chat_for_async.set_source_refs(source_count as usize);
                                        }
                                        chat_for_async.set_rag_trace(trace_json);
                                    }
                                }
                                SseEvent::Activity { .. } => {}
                                SseEvent::AnswerStart { .. } => {}
                                SseEvent::Token { content, .. } => {
                                    current_content.push_str(&content);
                                    chat_for_async.append_token(content);
                                    chat_for_async.update_streaming_message(
                                        current_content.clone(),
                                        current_citations.clone(),
                                    );
                                }
                                SseEvent::Citations { citations, .. } => {
                                    current_citations = typed_citations_from_values(citations);
                                    chat_for_async.set_source_refs(
                                        shared_chat_sources_from_citations(&current_citations)
                                            .len(),
                                    );
                                    chat_for_async.set_citations(current_citations.clone());
                                }
                                SseEvent::ReasoningSummaryDelta { .. } => {}
                                SseEvent::Done {
                                    session_id,
                                    message_id,
                                    payload,
                                    ..
                                } => {
                                    let done_citations = payload_citations(&payload);
                                    if let Some(ref citations) = done_citations {
                                        chat_for_async.set_source_refs(
                                            shared_chat_sources_from_citations(citations).len(),
                                        );
                                        chat_for_async.set_citations(citations.clone());
                                    }
                                    chat_for_async
                                        .set_degrade_trace(payload_degrade_reasons(&payload));
                                    chat_for_async.finalize_response(
                                        Some(session_id),
                                        Some(message_id),
                                        payload
                                            .get("answer")
                                            .and_then(|value| value.as_str())
                                            .map(str::to_string)
                                            .or_else(|| {
                                                (!current_content.is_empty())
                                                    .then_some(current_content.clone())
                                            }),
                                        payload_answer_blocks(&payload),
                                        done_citations,
                                    );
                                    set_is_submitting.set(false);
                                    break;
                                }
                                SseEvent::Error { message, .. } => {
                                    chat_for_async.set_error(message);
                                    set_is_submitting.set(false);
                                    break;
                                }
                            }
                        }
                    }
                    Err(error) => {
                        chat_for_async.set_error(format!(
                            "{}: {}",
                            choose(
                                locale.get_untracked(),
                                "启动对话失败",
                                "Failed to start chat"
                            ),
                            error
                        ));
                        set_is_submitting.set(false);
                    }
                }
            });
        }
    });

    let regenerate_message: Arc<dyn Fn(String) + Send + Sync> = Arc::new({
        let submit_query = submit_query.clone();
        let chat = chat.clone();
        move |assistant_message_id: String| {
            let messages = chat.messages.get_untracked();
            let Some(index) = messages
                .iter()
                .position(|message| message.id == assistant_message_id)
            else {
                return;
            };
            let Some(previous_user_message) = messages[..index]
                .iter()
                .rev()
                .find(|message| message.role == ChatRole::User)
            else {
                return;
            };
            submit_query(previous_user_message.content.clone());
        }
    });

    let on_edit_user =
        StoredValue::new(Arc::new(move |content: String| set_input_text.set(content))
            as Arc<dyn Fn(String) + Send + Sync>);
    let on_regenerate_assistant = StoredValue::new(Arc::new({
        let regenerate_message = regenerate_message.clone();
        move |message_id: String| regenerate_message(message_id)
    }) as Arc<dyn Fn(String) + Send + Sync>);
    let on_add_to_note = StoredValue::new(Arc::new(move |content: String| {
        append_to_note.with_value(|callback| callback(content));
    }) as Arc<dyn Fn(String) + Send + Sync>);

    view! {
        <div class=workspace_chat_style::shell>
            <div class=workspace_chat_style::hidden_status>
                <h2 class=workspace_chat_style::hidden_title>
                    {move || choose(locale.get(), "对话", "Chat")}
                </h2>
                <div class=workspace_chat_style::hidden_subtitle>
                    {move || status_label(locale.get(), chat.status.get())}
                </div>
            </div>

            <div
                class=workspace_chat_style::scroll
                data-test-chat-scroll
                on:scroll=move |_ev| {
                    #[cfg(target_arch = "wasm32")]
                    {
                        let container: web_sys::HtmlElement = event_target(&_ev);
                        _set_scroll_top_px.set(container.scroll_top() as f64);
                        set_viewport_height_px.set(container.client_height() as f64);
                    }
                }
            >
                <Show when=move || !chat.degrade_reasons.get().is_empty()>
                    <div class={format!("{} {}", workspace_chat_style::alert, workspace_chat_style::degrade_alert)}>
                        <div class=workspace_chat_style::alert_title>
                            {move || choose(locale.get(), "降级回答", "Degraded response")}
                        </div>
                        <div class=workspace_chat_style::alert_body>{move || chat.degrade_reasons.get().join(" | ")}</div>
                    </div>
                </Show>

                <Show when=move || chat.messages.get().is_empty()>
                    <div class=workspace_chat_style::empty_state_wrap>
                        <div class=workspace_chat_style::empty_state>
                            <svg class=workspace_chat_style::empty_icon fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"/>
                            </svg>
                            <p>{move || choose(locale.get(), "开始一个研究线程", "Start a conversation")}</p>
                            <p class=workspace_chat_style::empty_hint>
                                {move || choose(locale.get(), "围绕当前资料提问、追问，并把结论整理进笔记。", "Ask questions about your sources and capture conclusions into notes.")}
                            </p>
                        </div>
                    </div>
                </Show>

                <Show when=move || !chat.messages.get().is_empty()>
                    {move || {
                        let messages = chat.messages.get();
                        if messages.len() <= 24 {
                            view! {
                                <div class=workspace_chat_style::message_stack>
                                    {messages
                                        .into_iter()
                                        .map(|msg| {
                                            view! {
                                                <ChatBubble
                                                    message=msg
                                                    on_edit_user=on_edit_user
                                                    on_regenerate_assistant=on_regenerate_assistant
                                                    on_add_to_note=on_add_to_note
                                                />
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            }
                                .into_any()
                        } else {
                            view! {
                                <div class=workspace_chat_style::message_stack>
                                <VirtualTextList
                                    row_heights=Signal::derive(move || row_heights.get())
                                    viewport_height_px=Signal::derive(move || viewport_height_px.get())
                                    scroll_top_px=Signal::derive(move || scroll_top_px.get())
                                    overscan=CHAT_LIST_OVERSCAN
                                >
                                    <div class=workspace_chat_style::virtual_stack>
                                        {move || {
                                            let visible_ids = visible_ids.get();
                                            chat.messages
                                                .get()
                                                .into_iter()
                                                .filter(|message| visible_ids.iter().any(|id| id == &message.id))
                                                .map(|msg| {
                                                    view! {
                                                        <ChatBubble
                                                            message=msg
                                                            on_edit_user=on_edit_user
                                                            on_regenerate_assistant=on_regenerate_assistant
                                                            on_add_to_note=on_add_to_note
                                                        />
                                                    }
                                                })
                                                .collect_view()
                                        }}
                                    </div>
                                </VirtualTextList>
                                </div>
                            }
                                .into_any()
                        }
                    }}
                </Show>

                <Show when=move || chat.status.get() == ChatStatus::Error>
                    <div class={format!("{} {}", workspace_chat_style::alert, workspace_chat_style::error_alert)}>
                        <p class=workspace_chat_style::alert_title>{move || choose(locale.get(), "错误", "Error")}</p>
                        <p class=workspace_chat_style::alert_body>{move || chat.error_message.get().unwrap_or_default()}</p>
                    </div>
                </Show>
            </div>

            <div class=workspace_chat_style::compose_shell>
                <div class=workspace_chat_style::compose_card>
                    <form
                        on:submit={
                            let submit_query = submit_query.clone();
                            move |ev| {
                            ev.prevent_default();
                            submit_query(input_text.get());
                        }
                        }
                        class=workspace_chat_style::compose_form
                    >
                        <textarea
                            class=workspace_chat_style::compose_input
                            rows="1"
                            prop:value=move || input_text.get()
                            placeholder={move || choose(locale.get(), "输入问题，围绕当前资料继续研究...", "Ask a question about your sources...")}
                            on:input=move |ev| set_input_text.set(event_target_value(&ev))
                            on:keydown={
                                let submit_query = submit_query.clone();
                                move |ev| {
                                    if ev.key() == "Enter" && !ev.shift_key() {
                                        ev.prevent_default();
                                        submit_query(input_text.get());
                                    }
                                }
                            }
                            disabled=move || is_submitting.get()
                        ></textarea>
                        <div class=workspace_chat_style::compose_toolbar>
                            <div class=workspace_chat_style::mode_anchor>
                                <button
                                    type="button"
                                    class=workspace_chat_style::mode_trigger
                                    on:click=move |_| set_show_mode_menu.update(|open| *open = !*open)
                                >
                                    <span class=workspace_chat_style::mode_trigger_icon>{"+"}</span>
                                </button>
                                <span>{move || agent_mode_label(locale.get(), chat.agent_mode.get())}</span>
                                <Show when=move || show_mode_menu.get()>
                                    <div class={format!("workspace-menu {}", workspace_chat_style::mode_menu)}>
                                        <button
                                            type="button"
                                            class="workspace-menu-item"
                                            on:click=move |_| {
                                                chat.set_agent_mode.set(AgentMode::Rag);
                                                notebook_id_for_mode_write.with_value(|notebook_id| {
                                                    write_saved_agent_mode(notebook_id, AgentMode::Rag)
                                                });
                                                set_show_mode_menu.set(false);
                                            }
                                        >
                                            {"RAG"}
                                        </button>
                                        <button
                                            type="button"
                                            class="workspace-menu-item"
                                            on:click=move |_| {
                                                chat.set_agent_mode.set(AgentMode::General);
                                                notebook_id_for_mode_write.with_value(|notebook_id| {
                                                    write_saved_agent_mode(notebook_id, AgentMode::General)
                                                });
                                                set_show_mode_menu.set(false);
                                            }
                                        >
                                            {move || choose(locale.get(), "聊天", "Chat")}
                                        </button>
                                        <button
                                            type="button"
                                            class="workspace-menu-item"
                                            on:click=move |_| {
                                                chat.set_agent_mode.set(AgentMode::Search);
                                                notebook_id_for_mode_write.with_value(|notebook_id| {
                                                    write_saved_agent_mode(notebook_id, AgentMode::Search)
                                                });
                                                set_show_mode_menu.set(false);
                                            }
                                        >
                                            {move || choose(locale.get(), "网页", "Web")}
                                        </button>
                                    </div>
                                </Show>
                            </div>

                            <button
                                type="submit"
                                class=workspace_chat_style::send_button
                                disabled=move || input_text.get().trim().is_empty() || is_submitting.get()
                                title={move || choose(locale.get(), "Enter 发送，Shift+Enter 换行", "Enter sends, Shift+Enter adds a newline")}
                            >
                                <Show when=move || is_submitting.get()>
                                    <svg class=workspace_chat_style::send_spinner fill="none" viewBox="0 0 24 24">
                                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                    </svg>
                                </Show>
                                <Show when=move || !is_submitting.get()>
                                    <svg class=workspace_chat_style::send_icon fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 19l9 2-9-18-9 18 9-2zm0 0v-8"/>
                                    </svg>
                                </Show>
                            </button>
                        </div>
                    </form>
                </div>
            </div>
        </div>
    }
}
