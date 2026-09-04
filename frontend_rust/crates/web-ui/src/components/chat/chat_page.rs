use crate::components::chat::{ChatCanvasModel, PreparedUserTurn};
use crate::reducer::{ActivityEntry, TurnStatus};
use crate::session::{ConversationMessage, MessageRole};
use futures_util::StreamExt;
use leptos::prelude::*;
use leptos_router::NavigateOptions;
use leptos_router::hooks::{use_navigate, use_params};
use leptos_router::params::Params;
use web_sdk::{
    BrowserHttpTransport, ChatClient, ChatTransport, TauriIpcTransport, is_tauri_runtime,
};

#[derive(Params, PartialEq, Clone, Debug)]
struct ChatParams {
    session_id: Option<String>,
}

/// Chat-first 页面：/chat 与 /chat/:session_id 共用。
/// 模型信号由 App 级上下文提供；路由参数只负责会话绑定，不重建模型。
#[component]
pub fn ChatPage() -> impl IntoView {
    let model = expect_context::<RwSignal<ChatCanvasModel>>();
    // PoC token 信号放在 App 级上下文：路由切换重建页面后值仍保留（仅内存）。
    let token = expect_context::<RwSignal<String>>();
    let params = use_params::<ChatParams>();
    let navigate = use_navigate();

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
                model.update(|m| m.switch_to_personal_session(&session_id));
            }
        }
    });

    let current_token = move || {
        let value = token.get_untracked();
        (!value.is_empty()).then_some(value)
    };

    let try_prepare_turn = move |query: &str| {
        let query = query.trim().to_string();
        if query.is_empty() {
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
            let turn = model.write().retry_last();
            if let Some(turn) = turn {
                spawn_chat_stream(model, turn, current_token(), navigate.clone());
            }
            focus_composer(composer_ref);
        }
    };

    let is_streaming = move || model.with(|m| m.is_streaming());
    let can_retry = move || {
        model.with(|m| {
            !m.is_streaming()
                && m.manager()
                    .active
                    .messages
                    .iter()
                    .any(|message| message.role == MessageRole::User)
        })
    };

    view! {
        <main class="chat-canvas" aria-label="对话画布" data-testid="chat-canvas">
            <header class="chat-header">
                <h1 class="chat-title">"Context-OS 对话"</h1>
                <details class="poc-token">
                    <summary>"PoC 访问令牌（仅内存，不持久化）"</summary>
                    <label for="poc-token-input">"访问令牌"</label>
                    <input
                        id="poc-token-input"
                        data-testid="poc-token-input"
                        type="password"
                        autocomplete="off"
                        placeholder="留空则不携带 Authorization"
                        prop:value=move || token.get()
                        on:input=move |ev| token.set(event_target_value(&ev))
                    />
                </details>
            </header>

            <section class="chat-transcript" aria-label="消息列表" data-testid="chat-transcript">
                <Show when=move || model.with(|m| m.manager().active.messages.is_empty())>
                    <p class="chat-empty" data-testid="chat-empty">"开始一轮新的对话。"</p>
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
                        disabled=move || is_streaming()
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
    }
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

enum PocTransport {
    Browser(BrowserHttpTransport),
    Tauri(TauriIpcTransport),
}

impl ChatTransport for PocTransport {
    async fn stream_chat(
        &self,
        request: contracts::chat::ChatRequest,
        cancellation: web_sdk::Cancellation,
    ) -> Result<web_sdk::ChatEventStream, web_sdk::TransportError> {
        match self {
            Self::Browser(transport) => transport.stream_chat(request, cancellation).await,
            Self::Tauri(transport) => transport.stream_chat(request, cancellation).await,
        }
    }
}

fn select_transport(token: Option<String>) -> PocTransport {
    if is_tauri_runtime() {
        PocTransport::Tauri(TauriIpcTransport::new(token))
    } else {
        PocTransport::Browser(BrowserHttpTransport::new(&poc_api_base(), token))
    }
}

fn focus_composer(composer_ref: NodeRef<leptos::html::Textarea>) {
    if let Some(element) = composer_ref.get() {
        let _ = element.focus();
    }
}

/// 启动一条聊天流。浏览器：Fetch/SSE。Tauri WebView：既有 `chat_stream` IPC。
/// 两条路径都把 `ChatEvent` 交给同一 reducer。任务内只允许访问 App 级 model、
/// Router 级 navigate 与 window.location。
fn spawn_chat_stream(
    model: RwSignal<ChatCanvasModel>,
    turn: PreparedUserTurn,
    token: Option<String>,
    navigate: impl Fn(&str, NavigateOptions) + Clone + 'static,
) {
    leptos::task::spawn_local(async move {
        let client = ChatClient::new(select_transport(token));
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

/// 测试缝：隔离测试用显式 base URL；默认为空字符串（同源 /api/v1/chat）。
/// 仅浏览器端读取内存态全局变量，不读取/持久化任何凭据。
#[cfg(target_arch = "wasm32")]
fn poc_api_base() -> String {
    web_sys::window()
        .and_then(|window| {
            js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str("__POC_CHAT_API_BASE__"))
                .ok()
        })
        .and_then(|value| value.as_string())
        .map(|base| base.trim_end_matches('/').to_string())
        .unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
fn poc_api_base() -> String {
    String::new()
}
