use crate::api_base::poc_api_base;
use crate::components::chat::{ChatCanvasModel, PreparedUserTurn};
use crate::reducer::{ActivityEntry, TurnStatus};
use crate::session::{ConversationMessage, MessageRole, messages_from_wire};
use contracts::workspaces::ChatSession;
use futures_util::StreamExt;
use leptos::prelude::*;
use leptos_router::NavigateOptions;
use leptos_router::hooks::{use_navigate, use_params};
use leptos_router::params::Params;
use web_sdk::{BrowserHttpTransport, BrowserRestClient, ChatClient};

#[derive(Params, PartialEq, Clone, Debug)]
struct ChatParams {
    session_id: Option<String>,
}

/// Chat-first 页面：/chat 与 /chat/:session_id 共用。
/// 模型信号由 App 级上下文提供；路由参数只负责会话绑定，不重建模型。
#[component]
pub fn ChatPage() -> impl IntoView {
    let model = expect_context::<RwSignal<ChatCanvasModel>>();
    let token = expect_context::<RwSignal<String>>();
    let params = use_params::<ChatParams>();
    let navigate = use_navigate();
    let history_error = RwSignal::new(None::<String>);
    let history_loading = RwSignal::new(false);
    let history_load_gen = RwSignal::new(0_u64);

    let composer = RwSignal::new(String::new());
    let composer_ref = NodeRef::<leptos::html::Textarea>::new();

    // 路由参数 → 会话绑定（仅客户端 Effect 执行）。
    // 与 Next use-chat-session 同规则：参数等于当前流刚确立的 session id 时
    // 是 URL 落地的回显，不得再次切换打断流。
    // 注意：模型读取必须 untracked —— Effect 只应响应路由参数变化。若订阅了
    // model，新流确立 session id 的瞬间（URL 尚未更新）会误判成"切换到旧会话"，
    // 清空消息并作废当前流。
    Effect::new(move |_| {
        let session_id = params
            .read()
            .as_ref()
            .ok()
            .and_then(|p| p.session_id.clone());
        if let Some(session_id) = session_id {
            let needs_bind = model.with_untracked(|m| {
                m.manager().active.session_id.as_deref() != Some(session_id.as_str())
            });
            if needs_bind {
                model.update(|m| {
                    if let Some(session) = m
                        .manager()
                        .session_list
                        .iter()
                        .find(|session| session.id == session_id)
                        .cloned()
                    {
                        m.switch_to_session(&session);
                    } else {
                        m.switch_to_personal_session(&session_id);
                    }
                });
            }
            let token_value = token.get_untracked();
            let should_load = !token_value.is_empty()
                && model.with_untracked(|m| {
                    !m.is_streaming()
                        && m.manager().active.messages.is_empty()
                        && m.manager().active.session_id.as_deref() == Some(session_id.as_str())
                });
            if should_load {
                let epoch = model.with_untracked(|m| m.manager().conversation_epoch);
                spawn_load_history(
                    model,
                    session_id,
                    epoch,
                    token_value,
                    history_error,
                    history_loading,
                    history_load_gen,
                );
            }
        } else {
            let should_reset = model.with_untracked(|m| {
                !m.is_streaming() && m.manager().active.session_id.is_some()
            });
            if should_reset {
                model.update(|m| m.new_personal_chat(None));
                history_error.set(None);
                history_loading.set(false);
                history_load_gen.update(|n| *n += 1);
            }
        }
    });

    Effect::new(move |_| {
        let token_value = token.get();
        if token_value.is_empty() {
            model.update(|m| m.replace_session_list(Vec::new()));
            return;
        }
        spawn_refresh_sessions(model, Some(token_value.clone()));
        let session_id = params
            .get_untracked()
            .ok()
            .and_then(|p| p.session_id);
        let Some(session_id) = session_id else {
            return;
        };
        let should_load = model.with_untracked(|m| {
            !m.is_streaming()
                && m.manager().active.messages.is_empty()
                && m.manager().active.session_id.as_deref() == Some(session_id.as_str())
        });
        if should_load {
            let epoch = model.with_untracked(|m| m.manager().conversation_epoch);
            spawn_load_history(
                model,
                session_id,
                epoch,
                token_value,
                history_error,
                history_loading,
                history_load_gen,
            );
        }
    });

    let current_token = move || {
        let value = token.get_untracked();
        (!value.is_empty()).then_some(value)
    };

    let try_prepare_turn = move |query: &str| {
        let query = query.trim().to_string();
        if query.is_empty() || history_loading.get_untracked() {
            return None;
        }
        let mut model = model.write();
        if model.is_streaming() {
            None
        } else {
            Some(model.prepare_user_turn(&query))
        }
    };

    let send = {
        let navigate = navigate.clone();
        move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            let Some(turn) = try_prepare_turn(&composer.get()) else {
                return;
            };
            composer.set(String::new());
            spawn_chat_stream(model, turn, current_token(), navigate.clone());
            focus_composer(composer_ref);
        }
    };

    let send_keydown = {
        let navigate = navigate.clone();
        move |ev: leptos::ev::KeyboardEvent| {
            if ev.key() == "Enter" && !ev.shift_key() {
                ev.prevent_default();
                let Some(turn) = try_prepare_turn(&composer.get()) else {
                    return;
                };
                composer.set(String::new());
                spawn_chat_stream(model, turn, current_token(), navigate.clone());
            }
        }
    };

    let stop = move |_| {
        model.update(|m| {
            m.cancel();
        });
        focus_composer(composer_ref);
    };

    let retry = {
        let navigate = navigate.clone();
        move |_| {
            if history_loading.get_untracked() {
                return;
            }
            let turn = model.write().retry_last();
            if let Some(turn) = turn {
                spawn_chat_stream(model, turn, current_token(), navigate.clone());
            }
            focus_composer(composer_ref);
        }
    };

    let start_new = {
        let navigate = navigate.clone();
        move |_| {
            model.update(|m| m.new_personal_chat(None));
            history_error.set(None);
            history_loading.set(false);
            history_load_gen.update(|n| *n += 1);
            navigate(
                "/chat",
                NavigateOptions {
                    scroll: false,
                    ..Default::default()
                },
            );
        }
    };

    let open_session = {
        let navigate = navigate.clone();
        move |session_id: String| {
            navigate(
                &format!("/chat/{session_id}"),
                NavigateOptions {
                    scroll: false,
                    ..Default::default()
                },
            );
        }
    };

    let is_streaming = move || model.with(|m| m.is_streaming());
    let composer_locked = move || is_streaming() || history_loading.get();
    let can_retry = move || {
        model.with(|m| {
            !m.is_streaming()
                && !history_loading.get()
                && m.manager()
                    .active
                    .messages
                    .iter()
                    .any(|message| message.role == MessageRole::User)
        })
    };

    view! {
        <div class="chat-shell">
            <aside class="chat-sessions" aria-label="会话列表" data-testid="session-list">
                <button
                    type="button"
                    class="chat-new-chat"
                    data-testid="new-chat-button"
                    on:click=start_new
                >
                    "新对话"
                </button>
                <Show when=move || token.with(|value| value.is_empty())>
                    <p class="chat-sessions-hint" data-testid="session-auth-hint">"登录后即可加载会话"</p>
                </Show>
                <ul class="chat-session-items">
                    <For
                        each=move || model.with(|m| m.manager().session_list.clone())
                        key=|session| session.id.clone()
                        children=move |session| {
                            let session_id = session.id.clone();
                            let current_id = session_id.clone();
                            let open_session = open_session.clone();
                            view! {
                                <li>
                                    <button
                                        type="button"
                                        class="chat-session-item"
                                        data-testid="session-item"
                                        data-session-id=session.id.clone()
                                        data-current=move || {
                                            if model.with(|m| {
                                                m.manager().active.session_id.as_deref()
                                                    == Some(current_id.as_str())
                                            }) {
                                                "true"
                                            } else {
                                                "false"
                                            }
                                        }
                                        on:click=move |_| open_session(session_id.clone())
                                    >
                                        {session_label(&session)}
                                    </button>
                                </li>
                            }
                        }
                    />
                </ul>
                {move || {
                    history_error.get().map(|message| {
                        view! {
                            <p class="chat-error" role="alert" data-testid="session-error">
                                {message}
                            </p>
                        }
                    })
                }}
            </aside>
            <main class="chat-canvas" aria-label="对话画布" data-testid="chat-canvas">
                <header class="chat-header">
                    <h1 class="chat-title">"Context-OS 对话"</h1>
                </header>

                <section class="chat-transcript" aria-label="消息列表" data-testid="chat-transcript">
                    <Show when=move || {
                        model.with(|m| m.manager().active.messages.is_empty()) && !history_loading.get()
                    }>
                        <p class="chat-empty" data-testid="chat-empty">"开始一轮新的对话。"</p>
                    </Show>
                    <Show when=move || history_loading.get()>
                        <p class="chat-empty" data-testid="history-loading">"正在加载会话…"</p>
                    </Show>
                    <For
                        each=move || model.with(|m| m.manager().active.messages.clone())
                        key=|message| message.id.clone()
                        children=message_view
                    />

                    <Show when=move || {
                        model.with(|m| !matches!(m.live_turn().status, TurnStatus::Idle))
                    }>
                        <article class="chat-message chat-live" data-role="assistant">
                            <div
                                class="chat-live-answer"
                                aria-live="polite"
                                data-testid="live-answer"
                            >
                                {move || model.with(|m| m.live_turn().answer_text.clone())}
                            </div>
                        </article>
                    </Show>

                    {move || {
                        match model.with(|m| m.live_turn().status.clone()) {
                            TurnStatus::Error { code, message } => {
                                Some(view! {
                                    <p class="chat-error" role="alert" data-testid="chat-error">
                                        {format!("请求失败（{code}）：{message}")}
                                    </p>
                                })
                            }
                            _ => None,
                        }
                    }}

                    <Show when=move || model.with(|m| !m.live_turn().activities.is_empty())>
                        <section
                            class="chat-activity"
                            aria-label="进度"
                            aria-live="polite"
                            data-testid="activity-region"
                        >
                            <h2>"进度"</h2>
                            <ul>
                                <For
                                    each=move || model.with(|m| m.live_turn().activities.clone())
                                    key=|entry: &ActivityEntry| {
                                        format!("{}:{}", entry.phase, entry.title)
                                    }
                                    children=move |entry| {
                                        view! {
                                            <li>
                                                <span class="chat-activity-phase">{entry.phase}</span>
                                                <span class="chat-activity-title">{entry.title}</span>
                                                {entry.detail.map(|detail| view! {
                                                    <span class="chat-activity-detail">{detail}</span>
                                                })}
                                            </li>
                                        }
                                    }
                                />
                            </ul>
                        </section>
                    </Show>

                    <Show when=move || model.with(|m| !m.live_turn().reasoning_summary.is_empty())>
                        <section
                            class="chat-reasoning"
                            aria-label="推理摘要"
                            data-testid="reasoning-region"
                        >
                            <h2>"推理摘要"</h2>
                            <p>{move || model.with(|m| m.live_turn().reasoning_summary.clone())}</p>
                        </section>
                    </Show>

                    <Show when=move || model.with(|m| !m.live_turn().citations.is_empty())>
                        <section
                            class="chat-citations"
                            aria-label="引用"
                            data-testid="citations-region"
                        >
                            <h2>"引用"</h2>
                            <ul>
                                <For
                                    each=move || model.with(|m| m.live_turn().citations.clone())
                                    key=|citation| citation_key(citation)
                                    children=move |citation| {
                                        view! { <li>{citation_label(&citation)}</li> }
                                    }
                                />
                            </ul>
                        </section>
                    </Show>

                    <p class="chat-status" aria-live="polite" data-testid="status-line">
                        {move || model.with(status_line)}
                    </p>
                </section>

                <form class="chat-composer" aria-label="发送消息" on:submit=send>
                    <label for="chat-composer-input">"输入消息"</label>
                    <textarea
                        id="chat-composer-input"
                        data-testid="composer-input"
                        node_ref=composer_ref
                        rows=3
                        prop:value=move || composer.get()
                        on:input=move |ev| composer.set(event_target_value(&ev))
                        on:keydown=send_keydown
                        placeholder="输入消息，Enter 发送（Shift+Enter 换行）"
                    ></textarea>
                    <div class="chat-composer-actions">
                        <button
                            type="submit"
                            data-testid="send-button"
                            disabled=move || composer_locked()
                        >
                            "发送"
                        </button>
                        <button
                            type="button"
                            data-testid="stop-button"
                            disabled=move || !is_streaming()
                            on:click=stop
                        >
                            "停止"
                        </button>
                        <button
                            type="button"
                            data-testid="retry-button"
                            disabled=move || !can_retry()
                            on:click=retry
                        >
                            "重试"
                        </button>
                    </div>
                </form>
            </main>
        </div>
    }
}

fn session_label(session: &ChatSession) -> String {
    session
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(|title| title.to_string())
        .unwrap_or_else(|| "未命名对话".to_string())
}

fn message_view(message: ConversationMessage) -> impl IntoView {
    let role = match message.role {
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
    };
    let role_label = match message.role {
        MessageRole::User => "我",
        MessageRole::Assistant => "助手",
    };
    let reasoning = message.reasoning.clone();
    let citations = message.citations.clone();
    view! {
        <article class="chat-message" data-role=role data-testid="chat-message">
            <div class="chat-message-role">{role_label}</div>
            <div class="chat-message-content">{message.content}</div>
            {reasoning.map(|text| view! {
                <details class="chat-message-reasoning">
                    <summary>"推理摘要"</summary>
                    <p>{text}</p>
                </details>
            })}
            {(!citations.is_empty()).then(|| view! {
                <ul class="chat-message-citations">
                    {citations
                        .iter()
                        .map(|citation| view! { <li>{citation_label(citation)}</li> })
                        .collect::<Vec<_>>()}
                </ul>
            })}
        </article>
    }
}

fn status_line(model: &ChatCanvasModel) -> &'static str {
    match model.live_turn().status {
        TurnStatus::Idle => "",
        TurnStatus::Streaming => "正在生成回答…",
        TurnStatus::Done => "已完成",
        TurnStatus::Cancelled => "已停止",
        TurnStatus::Error { .. } => "",
    }
}

fn citation_key(citation: &serde_json::Value) -> String {
    citation
        .get("citation_id")
        .and_then(serde_json::Value::as_i64)
        .map(|id| format!("id:{id}"))
        .or_else(|| {
            citation
                .get("chunk_id")
                .and_then(serde_json::Value::as_str)
                .map(|id| format!("chunk:{id}"))
        })
        .unwrap_or_else(|| citation_label(citation))
}

fn citation_label(citation: &serde_json::Value) -> String {
    let name = citation
        .get("doc_name")
        .or_else(|| citation.get("title"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("来源");
    let preview = citation
        .get("preview")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty());
    match preview {
        Some(preview) => format!("{name} — {preview}"),
        None => name.to_string(),
    }
}

fn focus_composer(composer_ref: NodeRef<leptos::html::Textarea>) {
    if let Some(element) = composer_ref.get() {
        let _ = element.focus();
    }
}

/// 启动一条聊天流。浏览器 Fetch/SSE，事件进同一 reducer。
/// 任务内只允许访问 App 级 model、Router 级 navigate 与 window.location。
fn spawn_chat_stream(
    model: RwSignal<ChatCanvasModel>,
    turn: PreparedUserTurn,
    token: Option<String>,
    navigate: impl Fn(&str, NavigateOptions) + Clone + 'static,
) {
    leptos::task::spawn_local(async move {
        let client = ChatClient::new(BrowserHttpTransport::new(&poc_api_base(), token.clone()));
        let scope = turn.stream_scope;
        match client.stream(turn.request, turn.cancellation).await {
            Ok(mut stream) => {
                while let Some(item) = stream.next().await {
                    match item {
                        Ok(event) => {
                            let accepted = model.write().on_event(scope, event);
                            if accepted {
                                maybe_navigate_to_session(model, &navigate);
                            }
                        }
                        Err(error) => {
                            model.update(|m| {
                                m.on_transport_error(scope, error);
                            });
                        }
                    }
                }
            }
            Err(error) => {
                model.update(|m| {
                    m.on_transport_error(scope, error);
                });
            }
        }
        spawn_refresh_sessions(model, token);
    });
}

fn spawn_refresh_sessions(model: RwSignal<ChatCanvasModel>, token: Option<String>) {
    let Some(token) = token.filter(|value| !value.is_empty()) else {
        return;
    };
    leptos::task::spawn_local(async move {
        let client = BrowserRestClient::new(&poc_api_base(), Some(token));
        if let Ok(list) = client.list_sessions().await {
            model.update(|m| m.replace_session_list(list.sessions));
        }
    });
}

fn spawn_load_history(
    model: RwSignal<ChatCanvasModel>,
    session_id: String,
    epoch: u64,
    token: String,
    history_error: RwSignal<Option<String>>,
    history_loading: RwSignal<bool>,
    history_load_gen: RwSignal<u64>,
) {
    let load_gen = history_load_gen.get_untracked() + 1;
    history_load_gen.set(load_gen);
    history_loading.set(true);
    history_error.set(None);
    leptos::task::spawn_local(async move {
        let client = BrowserRestClient::new(&poc_api_base(), Some(token));
        let session = client.get_session(&session_id).await.ok();
        let result = client.list_messages(&session_id).await;
        if history_load_gen.get_untracked() != load_gen {
            return;
        }
        history_loading.set(false);
        match result {
            Ok(list) => {
                let applied = model.write().apply_history(
                    &session_id,
                    epoch,
                    session,
                    messages_from_wire(&list.messages),
                );
                if applied {
                    history_error.set(None);
                }
            }
            Err(error) => {
                history_error.set(Some(format!("加载会话失败：{error}")));
            }
        }
    });
}

/// 服务端 session id 落地后把地址更新为 /chat/:sessionId（replace，不重置流）。
/// 地址与当前会话一致时是本函数自己引发的回显，不再导航。
fn maybe_navigate_to_session(
    model: RwSignal<ChatCanvasModel>,
    navigate: &(impl Fn(&str, NavigateOptions) + Clone),
) {
    let Some(session_id) = model.with_untracked(|m| m.manager().active.session_id.clone()) else {
        return;
    };
    let target = format!("/chat/{session_id}");
    if current_path().as_deref() == Some(target.as_str()) {
        return;
    }
    navigate(
        &target,
        NavigateOptions {
            replace: true,
            scroll: false,
            ..Default::default()
        },
    );
}

#[cfg(target_arch = "wasm32")]
fn current_path() -> Option<String> {
    web_sys::window()?.location().pathname().ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn current_path() -> Option<String> {
    None
}
