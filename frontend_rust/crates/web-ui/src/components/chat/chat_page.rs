use crate::api_base::poc_api_base;
use crate::components::chat::{
    ChatCanvasModel, ChatComposer, MessageActions, PreparedUserTurn,
};
use crate::components::shell::{ContextTopBar, NavigationRail, NavigationState};
use crate::i18n::use_i18n;
use crate::routes::dest;
use crate::reducer::{ActivityEntry, TurnStatus};
use crate::session::{ConversationMessage, MessageRole, messages_from_wire};
use contracts::workspaces::ChatSession;
use futures_util::StreamExt;
use leptos::prelude::*;
use leptos_router::NavigateOptions;
use leptos_router::hooks::{query_signal, use_navigate, use_params};
use leptos_router::params::Params;
use web_sdk::{
    BrowserHttpTransport, BrowserRestClient, Capability, ChatClient, CitationView, SourceCard,
    activities_for_display, capabilities_to_wire, progress_folded, progress_summary_label,
    render_assistant_answer,
};

#[derive(Params, PartialEq, Clone, Debug)]
struct ChatParams {
    session_id: Option<String>,
    workspace_id: Option<String>,
}

#[derive(Clone, Copy)]
pub struct SharedChatModel(pub RwSignal<ChatCanvasModel>);

/// Chat-first 页面：/chat 与 /chat/:session_id 共用。
/// 模型信号由 App 级上下文提供；路由参数只负责会话绑定，不重建模型。
#[component]
pub fn ChatPage(
    #[prop(optional)] knowledge_scope: Option<Signal<Vec<String>>>,
    #[prop(optional)] selected_scope: Option<Signal<Vec<String>>>,
    #[prop(optional)] share_source: Option<Signal<(String, String)>>,
    #[prop(optional)] share_challenge: Option<crate::components::share::turnstile::ShareChallenge>,
) -> impl IntoView {
    let is_shared = share_source.is_some();
    let model = if is_shared { expect_context::<SharedChatModel>().0 } else { expect_context::<RwSignal<ChatCanvasModel>>() };
    if is_shared { on_cleanup(move || model.update(|m| { m.cancel(); })); }
    let token = expect_context::<RwSignal<String>>();
    let i18n = use_i18n();
    let params = use_params::<ChatParams>();
    let navigate = use_navigate();
    let history_error = RwSignal::new(None::<String>);
    let history_loading = RwSignal::new(false);
    let history_load_gen = RwSignal::new(0_u64);
    let sessions_error = RwSignal::new(None::<String>);
    let sessions_loading = RwSignal::new(false);

    let composer = RwSignal::new(String::new());
    let transcript_ref = NodeRef::<leptos::html::Section>::new();
    let elapsed_secs = RwSignal::new(0_u32);
    let navigation = NavigationState::new();
    let web_sources_open = RwSignal::new(false);
    let follow_bottom = RwSignal::new(true);
    let active_cite = RwSignal::new(None::<String>);
    provide_context(active_cite);
    let progress_expanded = RwSignal::new(false);
    let files_blocked = RwSignal::new(false);
    let attachments = RwSignal::new(Vec::<contracts::chat::TurnAttachment>::new());
    let attachment_error = RwSignal::new(false);
    let knowledge_chat = knowledge_scope.is_some();
    let ready_count = RwSignal::new(0_usize);
    let capabilities = RwSignal::new(Vec::<Capability>::new());
    let capabilities_manual = RwSignal::new(false);
    let last_scope = RwSignal::new(None::<Option<String>>);
    let has_byok = RwSignal::new(false);
    let current_model_role =
        Signal::derive(move || model.with(|m| m.manager().active.model_role.clone()));

    let (session_query, _) = query_signal::<String>("session");
    let route_workspace_id = Signal::derive(move || {
        if let Some(source) = share_source { return Some(source.get().1); }
        params
            .read()
            .as_ref()
            .ok()
            .and_then(|p| p.workspace_id.clone())
    });
    let route_session_id = Signal::derive(move || {
        if is_shared { return None; }
        let p_sid = params
            .read()
            .as_ref()
            .ok()
            .and_then(|p| p.session_id.clone());
        p_sid.or_else(|| session_query.get())
    });

    Effect::new(move |_| {
        if model.with(|m| matches!(m.live_turn().status, TurnStatus::Streaming)) {
            progress_expanded.set(false);
        }
    });

    Effect::new(move |_| {
        let streaming = model.with(|m| matches!(m.live_turn().status, TurnStatus::Streaming));
        if !streaming {
            return;
        }
        elapsed_secs.set(0);
        #[cfg(target_arch = "wasm32")]
        {
            if let Ok(handle) = set_interval_with_handle(
                move || elapsed_secs.update(|n| *n = n.saturating_add(1)),
                std::time::Duration::from_secs(1),
            ) {
                on_cleanup(move || handle.clear());
            }
        }
    });

    Effect::new(move |_| {
        let _chars = model.with(|m| m.live_turn().answer_text.len());
        let _n = model.with(|m| m.manager().active.messages.len());
        if follow_bottom.get() {
            scroll_transcript_to_bottom();
        }
    });

    // 路由参数 → 会话绑定（仅客户端 Effect 执行）。
    // 与 Next use-chat-session 同规则：参数等于当前流刚确立的 session id 时
    // 是 URL 落地的回显，不得再次切换打断流。
    // 注意：模型读取必须 untracked —— Effect 只应响应路由参数变化。若订阅了
    // model，新流确立 session id 的瞬间（URL 尚未更新）会误判成"切换到旧会话"，
    // 清空消息并作废当前流。
    Effect::new(move |_| {
        let ws_id = route_workspace_id.get();
        if let Some(source) = share_source {
            let (_, workspace_id) = source.get();
            model.update(|m| m.switch_to_workspace(&workspace_id, None));
            return;
        }
        let session_id = route_session_id.get();
        if let Some(ws_id) = ws_id {
            let needs_bind = model.with_untracked(|m| {
                m.manager().active.workspace_id.as_deref() != Some(ws_id.as_str())
                    || m.manager().active.session_id != session_id
            });
            if needs_bind {
                model.update(|m| {
                    if let Some(sid) = &session_id {
                        if let Some(session) = m
                            .manager()
                            .session_list
                            .iter()
                            .find(|session| session.id == *sid)
                            .cloned()
                        {
                            m.switch_to_session(&session);
                            return;
                        }
                    }
                    m.switch_to_workspace(&ws_id, session_id.as_deref());
                });
            }
        } else if let Some(session_id) = &session_id {
            let needs_bind = model.with_untracked(|m| {
                m.manager().active.session_id.as_deref() != Some(session_id.as_str())
                    || m.manager().active.workspace_id.is_some()
            });
            if needs_bind {
                model.update(|m| {
                    if let Some(session) = m
                        .manager()
                        .session_list
                        .iter()
                        .find(|session| session.id == *session_id)
                        .cloned()
                    {
                        m.switch_to_session(&session);
                    } else {
                        m.switch_to_personal_session(session_id);
                    }
                });
            }
        } else {
            let should_reset = model.with_untracked(|m| {
                m.manager().active.workspace_id.is_some()
                    || (!m.is_streaming() && m.manager().active.session_id.is_some())
            });
            if should_reset {
                model.update(|m| m.new_personal_chat(None));
                history_error.set(None);
                history_loading.set(false);
                history_load_gen.update(|n| *n += 1);
            }
        }

        if let Some(session_id) = session_id {
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
        }
    });

    Effect::new(move |_| {
        let token_value = token.get();
        if is_shared { return; }
        if token_value.is_empty() {
            model.update(|m| m.replace_session_list(Vec::new()));
            has_byok.set(false);
            return;
        }
        spawn_refresh_sessions(
            model,
            Some(token_value.clone()),
            Some((sessions_error, sessions_loading)),
        );
        spawn_check_byok(has_byok, token_value.clone());
        let session_id = route_session_id.get_untracked();
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

    // A workspace session opened through an old /chat URL belongs in its workspace.
    let navigate_workspace_session = navigate.clone();
    Effect::new(move |_| {
        if knowledge_chat { return; }
        let Some(requested) = route_session_id.get() else { return; };
        let destination = model.with(|m| {
            let active = &m.manager().active;
            (active.session_id.as_deref() == Some(requested.as_str()))
                .then(|| active.workspace_id.clone()).flatten()
        });
        if let Some(workspace) = destination {
            navigate_workspace_session(&format!("/dashboard/{workspace}?session={requested}"), NavigateOptions { replace: true, scroll: false, ..Default::default() });
        }
    });

    let current_token = move || {
        let value = token.get_untracked();
        (!value.is_empty()).then_some(value)
    };

    Effect::new(move |_| {
        let next = params
            .read()
            .as_ref()
            .ok()
            .and_then(|p| p.session_id.clone());
        let prev = last_scope.get_untracked();
        if prev.as_ref() == Some(&next) {
            return;
        }
        if let Some(prev_sid) = prev {
            let landing_from_new = prev_sid.is_none() && next.is_some();
            if !landing_from_new {
                capabilities.set(Vec::new());
                capabilities_manual.set(false);
                ready_count.set(0);
            }
        }
        last_scope.set(Some(next));
    });

    Effect::new(move |_| {
        let ready = knowledge_scope.map(|scope| scope.get().len()).unwrap_or(0);
        let _ = model.with(|m| m.manager().conversation_epoch);
        ready_count.set(ready);
        let manual = capabilities_manual.get_untracked();
        let current = capabilities.get_untracked();
        let mut next = current.clone();
        if !knowledge_chat || ready == 0 {
            next.retain(|cap| *cap != Capability::Rag);
        } else if !manual && !next.contains(&Capability::Rag) {
            next.insert(0, Capability::Rag);
        }
        if next != current {
            capabilities.set(next);
        }
    });

    let try_prepare_turn = move |query: &str| {
        let query = query.trim().to_string();
        if query.is_empty() || history_loading.get_untracked() || files_blocked.get_untracked()
            || share_challenge.is_some_and(|challenge| challenge.blocked_untracked()) {
            return None;
        }
        let caps = capabilities_to_wire(&capabilities.get_untracked());
        let files = attachments.get_untracked();
        if !files.is_empty() && query.len() + files.iter().map(|f| f.text.len() + f.filename.len()).sum::<usize>() > contracts::chat::MAX_TURN_CONTEXT_BYTES {
            attachment_error.set(true);
            return None;
        }
        attachment_error.set(false);
        let mut model = model.write();
        if model.is_streaming() {
            None
        } else {
            let mut turn = model.prepare_user_turn_with_attachments(&query, &caps, files);
            turn.request.doc_scope = selected_scope.map(|scope| scope.get_untracked()).unwrap_or_default();
            if let Some(source) = share_source {
                turn.request.source_type = Some("share".into());
                turn.request.source_token = Some(source.get_untracked().0);
                turn.request.turnstile_token = share_challenge.and_then(|challenge| challenge.consume());
                turn.request.messages = model.manager().active.messages.iter().rev().skip(1).rev().map(|message| contracts::chat::ChatTurnInput {
                    role: if message.role == MessageRole::User { "user" } else { "assistant" }.into(),
                    content: message.content.clone(), resolved_query: None,
                }).collect();
            }
            Some(turn)
        }
    };

    let send = {
        let navigate = navigate.clone();
        move |()| {
            let Some(turn) = try_prepare_turn(&composer.get()) else {
                return;
            };
            composer.set(String::new());
            attachments.set(Vec::new());
            follow_bottom.set(true);
            spawn_chat_stream(model, turn, current_token(), navigate.clone());
        }
    };

    let stop = move |_| {
        model.update(|m| {
            m.cancel();
        });
    };

    let retry = {
        let navigate = navigate.clone();
        move |_| {
            if history_loading.get_untracked() || files_blocked.get_untracked()
                || share_challenge.is_some_and(|challenge| challenge.blocked_untracked()) {
                return;
            }
            let caps = capabilities_to_wire(&capabilities.get_untracked());
            let turn = model.write().retry_last_with(&caps);
            if let Some(mut turn) = turn {
                turn.request.doc_scope = selected_scope.map(|scope| scope.get_untracked()).unwrap_or_default();
                if let Some(source) = share_source {
                    turn.request.source_type = Some("share".into());
                    turn.request.source_token = Some(source.get_untracked().0);
                    turn.request.turnstile_token = share_challenge.and_then(|challenge| challenge.consume());
                    turn.request.messages = model.with_untracked(|m| m.manager().active.messages.iter().rev().skip(1).rev().map(|message| contracts::chat::ChatTurnInput {
                        role: if message.role == MessageRole::User { "user" } else { "assistant" }.into(),
                        content: message.content.clone(), resolved_query: None,
                    }).collect());
                }
                follow_bottom.set(true);
                spawn_chat_stream(model, turn, current_token(), navigate.clone());
            }
        }
    };

    let start_new = {
        let navigate = navigate.clone();
        move |_| {
            navigation.open.set(false);
            model.update(|m| {
                if let Some(ws_id) = route_workspace_id.get_untracked() {
                    m.switch_to_workspace(&ws_id, None);
                } else {
                    m.new_personal_chat(None);
                }
            });
            capabilities.set(Vec::new());
            capabilities_manual.set(false);
            ready_count.set(0);
            history_error.set(None);
            history_loading.set(false);
            history_load_gen.update(|n| *n += 1);
            if let Some(ws_id) = route_workspace_id.get_untracked() {
                navigate(
                    &format!("/dashboard/{ws_id}"),
                    NavigateOptions {
                        scroll: false,
                        ..Default::default()
                    },
                );
            } else {
                navigate(
                    "/chat",
                    NavigateOptions {
                        scroll: false,
                        ..Default::default()
                    },
                );
            }
        }
    };

    let open_session = {
        let navigate = navigate.clone();
        move |session: ChatSession| {
            if model.with(|m| m.is_streaming()) {
                return;
            }
            navigation.open.set(false);
            if session.scope_kind == contracts::workspaces::ConversationScopeKind::Workspace {
                if let Some(ws_id) = &session.workspace_id {
                    navigate(
                        &format!("/dashboard/{}?session={}", ws_id, session.id),
                        NavigateOptions {
                            scroll: false,
                            ..Default::default()
                        },
                    );
                    return;
                }
            }
            navigate(
                &format!("/chat/{}", session.id),
                NavigateOptions {
                    scroll: false,
                    ..Default::default()
                },
            );
        }
    };

    let is_streaming = move || model.with(|m| m.is_streaming());
    let composer_locked = move || is_streaming() || history_loading.get() || files_blocked.get()
        || share_challenge.is_some_and(|challenge| challenge.blocked());
    let attach_disabled = Signal::derive(move || {
        token.get().is_empty() || is_streaming() || history_loading.get()
    });
    let can_retry = move || {
        model.with(|m| {
            !m.is_streaming()
                && m.has_retry_context()
                && !history_loading.get()
                && !files_blocked.get()
                && !share_challenge.is_some_and(|challenge| challenge.blocked())
                && m.manager()
                    .active
                    .messages
                    .iter()
                    .any(|message| message.role == MessageRole::User)
        })
    };

    let show_app_bar = Signal::derive(move || route_workspace_id.get().is_none());

    view! {
        <div class=move || if show_app_bar.get() { "app-layout" } else { "chat-embed" }
            data-navigation-open=move || navigation.open.get().to_string()
            data-navigation-collapsed=move || navigation.collapsed.get().to_string()>
        <div class="chat-shell">
            {(!knowledge_chat).then(|| view! {
            <NavigationRail state=navigation>
                {move || {
                    route_workspace_id.get().map(|ws_id| {
                        view! {
                            <div class="chat-workspace-banner" data-testid="workspace-banner">
                                <span class="chat-workspace-label">{i18n.t("chat.workspaces")}</span>
                                <span class="chat-workspace-id">{ws_id}</span>
                                <a href="/chat" class="chat-workspace-back" data-testid="back-to-personal">
                                    {i18n.t("chat.backToPersonal")}
                                </a>
                            </div>
                        }
                    })
                }}
                <button
                    type="button"
                    class="chat-new-chat"
                    data-testid="new-chat-button"
                    on:click=start_new
                >
                    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="M12 5v14M5 12h14"/></svg>
                    {move || i18n.t("chat.newConversation")}
                </button>
                <div class="chat-workspaces" data-testid="chat-workspaces">
                    <a href=dest::DASHBOARD class="chat-workspaces-all" data-testid="all-workspaces-link">
                        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="M3 7V4h7l2 3h9v13H3z"/></svg>
                        {move || i18n.t("navigation.workspace")}
                    </a>
                </div>
                <p class="chat-sessions-hint">{move || i18n.t("navigation.history")}</p>
                <Show when=move || token.with(|value| value.is_empty())>
                    <p class="chat-sessions-hint" data-testid="session-auth-hint">{move || i18n.t("chat.signInToLoadSessions")}</p>
                </Show>
                <Show when=move || sessions_loading.get()>
                    <p class="chat-sessions-hint" data-testid="session-loading">{move || i18n.t("chat.sessionsLoading")}</p>
                </Show>
                <Show when=move || {
                    !token.with(|value| value.is_empty())
                        && !sessions_loading.get()
                        && sessions_error.get().is_none()
                        && model.with(|m| m.manager().session_list.is_empty())
                }>
                    <p class="chat-sessions-hint" data-testid="session-empty">{move || i18n.t("chat.noSessions")}</p>
                </Show>
                {move || {
                    sessions_error.get().map(|message| {
                        view! {
                            <div class="chat-session-error" data-testid="session-list-error">
                                <p class="chat-error" role="alert">{message}</p>
                                <button
                                    type="button"
                                    class="chat-action-button"
                                    data-testid="session-retry"
                                    on:click=move |_| {
                                        spawn_refresh_sessions(
                                            model,
                                            current_token(),
                                            Some((sessions_error, sessions_loading)),
                                        );
                                    }
                                >
                                    {move || i18n.t("chat.retry")}
                                </button>
                            </div>
                        }
                    })
                }}
                <ul class="chat-session-items">
                    <For
                        each=move || model.with(|m| m.manager().session_list.clone())
                        key=|session| session.id.clone()
                        children=move |session| {
                            let item_session = session.clone();
                            let current_id = session.id.clone();
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
                                        disabled=move || is_streaming()
                                        on:click=move |_| open_session(item_session.clone())
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
            </NavigationRail>
            })}
            <div class="app-layout-main" inert=move || navigation.open.get()>
            <Show when=move || show_app_bar.get()>
                <ContextTopBar state=navigation title=Signal::derive(move || i18n.t("chat.pageTitle"))/>
            </Show>
            <main
                class=move || {
                    if model.with(|m| m.manager().active.messages.is_empty()) && !history_loading.get()
                    {
                        "chat-canvas is-hero"
                    } else {
                        "chat-canvas"
                    }
                }
                aria-label=move || i18n.t("chat.canvasLabel")
                data-testid="chat-canvas"
            >
                <section
                    class="chat-transcript"
                    aria-label=move || i18n.t("chat.transcriptLabel")
                    data-testid="chat-transcript"
                    node_ref=transcript_ref
                    on:scroll=move |_| on_transcript_scroll(transcript_ref, follow_bottom)
                >
                    <Show when=move || {
                        model.with(|m| m.manager().active.messages.is_empty()) && !history_loading.get()
                    }>
                        <div class="chat-hero" data-testid="chat-hero">
                            <p class="chat-hero-title">"Context-OS"</p>
                            <p class="chat-hero-hint" data-testid="chat-empty">
                                {move || i18n.t(if knowledge_chat { "chat.heroEmpty" } else { "chat.heroSubtitle" })}
                            </p>
                        </div>
                    </Show>
                    <Show when=move || history_loading.get()>
                        <p class="chat-empty" data-testid="history-loading">{move || i18n.t("chat.historyLoading")}</p>
                    </Show>
                    <For
                        each=move || model.with(|m| m.manager().active.messages.clone())
                        key=|message| message.id.clone()
                        children=move |message| message_view(message, composer, web_sources_open)
                    />

                    <Show when=move || {
                        model.with(|m| !matches!(m.live_turn().status, TurnStatus::Idle))
                    }>
                        <article class="chat-message chat-live" data-role="assistant">
                            <div
                                class=move || {
                                    if model.with(|m| {
                                        matches!(m.live_turn().status, TurnStatus::Streaming)
                                    }) {
                                        "chat-md chat-live-answer is-streaming"
                                    } else {
                                        "chat-md chat-live-answer"
                                    }
                                }
                                aria-live="polite"
                                data-testid="live-answer"
                                data-source-chars=move || {
                                    model
                                        .with(|m| m.live_turn().answer_text.chars().count())
                                        .to_string()
                                }
                                inner_html=move || live_rendered(&model).html
                                on:click=move |ev| on_markdown_click(ev, active_cite)
                            ></div>
                        </article>
                    </Show>

                    {move || {
                        let turn = model.with(|m| m.live_turn().clone());
                        notice_view(turn.degrade_reasons, turn.guarded)
                    }}
                    {move || tool_cards_view(model.with(|m| m.live_turn().tool_results.clone()))}
                    {move || {
                        web_sources_button(
                            model.with(|m| m.live_turn().citations.clone()),
                            web_sources_open,
                        )
                    }}

                    {move || {
                        match model.with(|m| m.live_turn().status.clone()) {
                            TurnStatus::Error { code, message } => {
                                Some(view! {
                                    <p class="chat-error" role="alert" data-testid="chat-error">
                                        {crate::i18n::tf_now(
                                            "chat.requestFailed",
                                            &[("code", &code), ("message", &message)],
                                        )}
                                    </p>
                                })
                            }
                            _ => None,
                        }
                    }}

                    <Show when=move || model.with(|m| !m.live_turn().activities.is_empty())>
                        <section
                            class="chat-activity"
                            aria-label=move || i18n.t("chat.progressLabel")
                            aria-live=move || {
                                if model.with(|m| progress_folded(&m.live_turn().status)) {
                                    "off"
                                } else {
                                    "polite"
                                }
                            }
                            data-collapsed=move || {
                                if model.with(|m| progress_folded(&m.live_turn().status))
                                    && !progress_expanded.get()
                                {
                                    "true"
                                } else {
                                    "false"
                                }
                            }
                            data-testid="activity-region"
                        >
                            <Show when=move || {
                                model.with(|m| progress_folded(&m.live_turn().status))
                            }>
                                <button
                                    type="button"
                                    class="chat-progress-toggle"
                                    data-testid="progress-toggle"
                                    aria-expanded=move || progress_expanded.get()
                                    on:click=move |_| {
                                        progress_expanded.update(|open| *open = !*open);
                                    }
                                >
                                    <span class="chat-progress-summary">
                                        {move || {
                                            model.with(|m| {
                                                {
                                                    let key = progress_summary_label(&m.live_turn().status);
                                                    if key.is_empty() {
                                                        String::new()
                                                    } else {
                                                        i18n.t(key)
                                                    }
                                                }
                                            })
                                        }}
                                    </span>
                                    <span class="chat-progress-chevron" aria-hidden="true">
                                        {move || {
                                            if progress_expanded.get() { "▾" } else { "▸" }
                                        }}
                                    </span>
                                </button>
                            </Show>
                            <Show when=move || {
                                !model.with(|m| progress_folded(&m.live_turn().status))
                            }>
                                <h2>{move || i18n.t("chat.progressLabel")}</h2>
                            </Show>
                            <Show when=move || {
                                !model.with(|m| progress_folded(&m.live_turn().status))
                                    || progress_expanded.get()
                            }>
                                <ul data-testid="activity-steps">
                                    <For
                                        each=move || {
                                            model.with(|m| {
                                                activities_for_display(&m.live_turn().activities)
                                            })
                                        }
                                        key=|entry: &ActivityEntry| {
                                            format!("{}:{}:{}", entry.phase, entry.title, entry.detail.as_deref().unwrap_or(""))
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
                            </Show>
                        </section>
                    </Show>

                    <Show when=move || model.with(|m| !m.live_turn().reasoning_summary.is_empty())>
                        {move || {
                            let text = model.with(|m| m.live_turn().reasoning_summary.clone());
                            if model.with(|m| progress_folded(&m.live_turn().status)) {
                                view! {
                                    <details
                                        class="chat-reasoning"
                                        aria-label=move || i18n.t("chat.reasoningSummary")
                                        data-testid="reasoning-region"
                                    >
                                        <summary>{move || i18n.t("chat.reasoningSummary")}</summary>
                                        <p>{text}</p>
                                    </details>
                                }
                                .into_any()
                            } else {
                                view! {
                                    <section
                                        class="chat-reasoning"
                                        aria-label=move || i18n.t("chat.reasoningSummary")
                                        data-testid="reasoning-region"
                                    >
                                        <h2>{move || i18n.t("chat.reasoningSummary")}</h2>
                                        <p>{text}</p>
                                    </section>
                                }
                                .into_any()
                            }
                        }}
                    </Show>

                    <Show when=move || !live_rendered(&model).cards.is_empty()>
                        <section
                            class="chat-citations"
                            aria-label=move || i18n.t("chat.citationsLabel")
                            data-testid="citations-region"
                        >
                            <h2>{move || i18n.t("chat.citationsLabel")}</h2>
                            <ul class="chat-cite-cards">
                                <For
                                    each=move || live_rendered(&model).cards
                                    key=|card| card.key.clone()
                                    children=move |card| source_card_view(card, active_cite)
                                />
                            </ul>
                        </section>
                    </Show>

                    <div class="chat-status-row">
                        <p class="chat-status" aria-live="polite" data-testid="status-line">
                            {move || model.with(status_line)}
                        </p>
                        <span
                            class="chat-elapsed"
                            data-testid="workspace-progress-elapsed"
                            hidden=move || {
                                model.with(|m| matches!(m.live_turn().status, TurnStatus::Idle))
                            }
                        >
                            {move || format_elapsed(elapsed_secs.get())}
                        </span>
                    </div>
                </section>

                <ChatComposer
                    model_role=current_model_role
                    has_byok=Signal::derive(move || has_byok.get())
                    value=composer
                    streaming=Signal::derive(is_streaming)
                    locked=Signal::derive(composer_locked)
                    can_retry=Signal::derive(can_retry)
                    history_loading=history_loading
                    files_blocked=files_blocked
                    ready_count=ready_count
                    knowledge_chat=knowledge_chat
                    attachments=attachments
                    epoch=Signal::derive(move || model.with(|m| m.manager().conversation_epoch))
                    attach_disabled=attach_disabled
                    capabilities=capabilities
                    capabilities_manual=capabilities_manual
                    on_submit=Callback::new(send)
                    on_stop=Callback::new(stop)
                    on_retry=Callback::new(retry)
                />
                <Show when=move || attachment_error.get()>
                    <p role="alert" class="chat-file-error">{move || i18n.t("chat.attachmentContextLimit")}</p>
                </Show>
                <button
                    type="button"
                    class="chat-scroll-bottom"
                    data-testid="scroll-to-bottom"
                    hidden=move || follow_bottom.get()
                    on:click=move |_| {
                        follow_bottom.set(true);
                        scroll_transcript_to_bottom();
                    }
                >
                    {move || i18n.t("workspaceChatBackToBottom")}
                </button>
                <Show when=move || web_sources_open.get()>
                    {move || {
                        let sources = collect_web_sources(
                            &model.with(|m| {
                                m.live_turn()
                                    .citations
                                    .clone()
                                    .into_iter()
                                    .chain(
                                        m.manager()
                                            .active
                                            .messages
                                            .iter()
                                            .flat_map(|message| message.citations.clone()),
                                    )
                                    .collect::<Vec<_>>()
                            }),
                        );
                        web_sources_dialog(sources, web_sources_open)
                    }}
                </Show>
            </main>
            </div>
        </div>
        </div>
    }
}

fn session_label(session: &ChatSession) -> String {
    let i18n = use_i18n();
    let base = session
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(|title| title.to_string())
        .unwrap_or_else(|| i18n.t("chat.untitled"));
    if session.scope_kind == contracts::workspaces::ConversationScopeKind::Workspace {
        let fallback = i18n.t("chat.workspaces");
        let ws_name = session
            .workspace_name
            .as_deref()
            .unwrap_or(fallback.as_str());
        format!("[{ws_name}] {base}")
    } else {
        base
    }
}

fn message_view(
    message: ConversationMessage,
    composer: RwSignal<String>,
    web_sources_open: RwSignal<bool>,
) -> impl IntoView {
    let active_cite = expect_context::<RwSignal<Option<String>>>();
    let role = match message.role {
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
    };
    let i18n = use_i18n();
    let role_label = match message.role {
        MessageRole::User => i18n.t("chat.roleUser"),
        MessageRole::Assistant => i18n.t("chat.roleAssistant"),
    };
    let reasoning = message.reasoning.clone();
    let rendered = if message.role == MessageRole::Assistant {
        Some(render_message_answer(&message.content, &message.citations))
    } else {
        None
    };
    let html = rendered.as_ref().map(|value| value.html.clone());
    let cards = rendered.map(|value| value.cards).unwrap_or_default();
    let edit_text = message.content.clone();
    let is_user = message.role == MessageRole::User;
    view! {
        <article class="chat-message" data-role=role data-testid="chat-message">
            <div class="chat-message-role">{role_label}</div>
            {(!message.attachment_names.is_empty()).then(|| view! {
                <p class="chat-composer-hint" data-testid="message-attachments">{i18n.tf("chat.attachmentHistory", &[("names", &message.attachment_names.join(", "))])}</p>
            })}
            {if let Some(html) = html {
                view! {
                    <div
                        class="chat-md chat-message-content"
                        inner_html=html
                        on:click=move |ev| on_markdown_click(ev, active_cite)
                    ></div>
                }
                .into_any()
            } else {
                view! {
                    <div class="chat-message-content">{message.content.clone()}</div>
                }
                .into_any()
            }}
            {is_user.then(|| {
                view! {
                    <button
                        type="button"
                        class="chat-action-button"
                        data-testid="edit-user-message"
                        on:click=move |_| composer.set(edit_text.clone())
                    >
                        {move || i18n.t("workspaceChatActionEdit")}
                    </button>
                }
            })}
            {notice_view(message.degrade_reasons.clone(), message.guarded)}
            {tool_cards_view(message.tool_results.clone())}
            {web_sources_button(message.citations.clone(), web_sources_open)}
            {reasoning.map(|text| view! {
                <details class="chat-message-reasoning">
                    <summary>{i18n.t("chat.reasoningSummary")}</summary>
                    <p>{text}</p>
                </details>
            })}
            {(!cards.is_empty()).then(|| view! {
                <ul class="chat-cite-cards chat-message-citations">
                    {cards
                        .into_iter()
                        .map(|card| source_card_view(card, active_cite))
                        .collect::<Vec<_>>()}
                </ul>
            })}
            {(message.role == MessageRole::Assistant).then(|| {
                view! {
                    <MessageActions
                        content=message.content.clone()
                        session_id=message.session_id.clone()
                        message_id=message.message_id
                    />
                }
            })}
        </article>
    }
}

fn status_line(model: &ChatCanvasModel) -> String {
    let i18n = use_i18n();
    match model.live_turn().status {
        TurnStatus::Idle => String::new(),
        TurnStatus::Streaming => i18n.t("chat.statusStreaming"),
        TurnStatus::Done => i18n.t("chat.statusDone"),
        TurnStatus::Cancelled => i18n.t("chat.statusCancelled"),
        TurnStatus::Error { .. } => String::new(),
    }
}

fn live_rendered(model: &RwSignal<ChatCanvasModel>) -> web_sdk::RenderedAnswer {
    model.with(|m| render_message_answer(&m.live_turn().answer_text, &m.live_turn().citations))
}

fn render_message_answer(text: &str, citations: &[serde_json::Value]) -> web_sdk::RenderedAnswer {
    let cites: Vec<CitationView> = citations.iter().map(CitationView::from_value).collect();
    render_assistant_answer(text, &cites)
}

fn source_card_view(card: SourceCard, active_cite: RwSignal<Option<String>>) -> impl IntoView {
    let key = card.key.clone();
    let key_attr = card.key.clone();
    let seq = card.seq.to_string();
    let title = card.title.clone();
    let preview = card.preview.clone();
    let href = card.href.clone();
    let tombstone = card.tombstone;
    view! {
        <li
            class=move || {
                if active_cite.get().as_deref() == Some(key.as_str()) {
                    "chat-cite-card is-active"
                } else {
                    "chat-cite-card"
                }
            }
            data-cite-card=key_attr
            data-testid="citation-card"
        >
            <span class="chat-cite-card-seq">{seq}</span>
            <div class="chat-cite-card-body">
                <div class="chat-cite-card-title">{title}</div>
                {(!preview.is_empty()).then(|| view! {
                    <p class="chat-cite-card-preview">{preview}</p>
                })}
                {href.map(|href| view! {
                    <a class="chat-cite-card-link" href=href rel="noopener noreferrer" target="_blank">
                        {use_i18n().t("chat.openSource")}
                    </a>
                })}
                {tombstone.then(|| view! {
                    <p class="chat-cite-card-tombstone">{use_i18n().t("chat.sourceDeleted")}</p>
                })}
            </div>
        </li>
    }
}

fn on_markdown_click(ev: leptos::ev::MouseEvent, active_cite: RwSignal<Option<String>>) {
    if copy_code_from_click(&ev) {
        return;
    }
    on_citation_chip_click(ev, active_cite);
}

fn on_citation_chip_click(ev: leptos::ev::MouseEvent, active_cite: RwSignal<Option<String>>) {
    let Some(key) = citation_key_from_click(&ev) else {
        return;
    };
    active_cite.set(Some(key.clone()));
    scroll_cite_card(&key);
}

fn notice_view(reasons: Vec<String>, guarded: bool) -> impl IntoView {
    let i18n = use_i18n();
    let joined = reasons.join(" · ");
    let body = if guarded && !reasons.is_empty() {
        i18n.tf("chat.guardedWithReasons", &[("reasons", joined.as_str())])
    } else if guarded {
        i18n.t("chat.guarded")
    } else if !reasons.is_empty() {
        i18n.tf("chat.degradeReasons", &[("reasons", joined.as_str())])
    } else {
        String::new()
    };
    let show = !body.is_empty();
    view! {
        <p class="chat-notice" data-testid="chat-degrade-notice" role="status" hidden=!show>
            {body}
        </p>
    }
}

fn tool_cards_view(results: Vec<serde_json::Value>) -> impl IntoView {
    let cards: Vec<_> = results
        .iter()
        .map(|result| {
            (
                result
                    .get("tool")
                    .and_then(|value| value.as_str())
                    .unwrap_or("tool")
                    .to_string(),
                result
                    .get("status")
                    .and_then(|value| value.as_str())
                    .unwrap_or("ok")
                    .to_string(),
                result
                    .get("data")
                    .map(|data| {
                        serde_json::to_string_pretty(data).unwrap_or_else(|_| data.to_string())
                    })
                    .unwrap_or_default(),
            )
        })
        .collect();
    let empty = cards.is_empty();
    view! {
        <ul class="chat-tool-list" data-testid="tool-result-list" hidden=empty>
            {cards
                .into_iter()
                .map(|(tool, status, summary)| {
                    view! {
                        <li class="chat-tool-card" data-testid="tool-result-card">
                            <header>
                                <strong>{tool}</strong>
                                <span>{status}</span>
                            </header>
                            <pre>{summary}</pre>
                        </li>
                    }
                })
                .collect_view()}
        </ul>
    }
}

fn collect_web_sources(citations: &[serde_json::Value]) -> Vec<(String, String, String)> {
    let mut seen = std::collections::BTreeSet::new();
    citations
        .iter()
        .filter_map(|value| {
            let view = CitationView::from_value(value);
            let url = view.url.clone()?;
            if !seen.insert(url.clone()) {
                return None;
            }
            Some((
                if view.doc_name.is_empty() {
                    url.clone()
                } else {
                    view.doc_name
                },
                url,
                view.preview.unwrap_or_default(),
            ))
        })
        .collect()
}

fn web_sources_button(
    citations: Vec<serde_json::Value>,
    open: RwSignal<bool>,
) -> impl IntoView {
    let count = collect_web_sources(&citations).len();
    (count > 0).then(|| {
        let count_s = count.to_string();
        let label = use_i18n().tf("chat.webSources", &[("count", count_s.as_str())]);
        view! {
            <button
                type="button"
                class="chat-action-button"
                data-testid="web-sources-button"
                on:click=move |_| open.set(true)
            >
                {label}
            </button>
        }
    })
}

fn web_sources_dialog(
    sources: Vec<(String, String, String)>,
    open: RwSignal<bool>,
) -> impl IntoView {
    let i18n = use_i18n();
    let count_s = sources.len().to_string();
    let heading = i18n.tf("chat.webSources", &[("count", count_s.as_str())]);
    view! {
        <crate::components::ui::AppDialog open=Signal::derive(move || open.get()) title_key="chat.webSourcesLabel" test_id="workspace-web-sources-modal" on_close=Callback::new(move |_| open.set(false))>
                <p>{heading}</p>
                <ul class="chat-web-source-list" data-testid="workspace-web-sources-list">
                    {sources
                        .into_iter()
                        .map(|(title, url, snippet)| {
                            let snippet_empty = snippet.is_empty();
                            view! {
                                <li class="chat-web-source">
                                    <a href=url.clone() rel="noopener noreferrer" target="_blank">
                                        {title}
                                    </a>
                                    <p>{url}</p>
                                    <p hidden=snippet_empty>{snippet}</p>
                                </li>
                            }
                        })
                        .collect_view()}
                </ul>
        </crate::components::ui::AppDialog>
    }
}

#[cfg(target_arch = "wasm32")]
fn copy_code_from_click(ev: &leptos::ev::MouseEvent) -> bool {
    use wasm_bindgen::JsCast;
    let Some(target) = ev.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) else {
        return false;
    };
    let Ok(Some(button)) = target.closest("[data-testid=\"chat-code-copy\"]") else {
        return false;
    };
    let Some(block) = button.closest(".chat-code-block").ok().flatten() else {
        return false;
    };
    let Ok(Some(code)) = block.query_selector("code") else {
        return false;
    };
    let text = code.text_content().unwrap_or_default();
    if let Some(window) = web_sys::window() {
        let _ = window.navigator().clipboard().write_text(&text);
    }
    true
}

#[cfg(not(target_arch = "wasm32"))]
fn copy_code_from_click(_ev: &leptos::ev::MouseEvent) -> bool {
    false
}

fn format_elapsed(total_seconds: u32) -> String {
    if total_seconds < 60 {
        format!("{total_seconds}s")
    } else {
        format!("{}m {}s", total_seconds / 60, total_seconds % 60)
    }
}

fn on_transcript_scroll(
    transcript_ref: NodeRef<leptos::html::Section>,
    follow_bottom: RwSignal<bool>,
) {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(node) = transcript_ref.get() else {
            return;
        };
        let el: &web_sys::HtmlElement = &node;
        let remaining = el.scroll_height() - el.scroll_top() - el.client_height();
        follow_bottom.set(remaining < 64);
    }
    let _ = (transcript_ref, follow_bottom);
}

fn scroll_transcript_to_bottom() {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
            if let Ok(Some(scroller)) = document.query_selector("[data-testid=\"chat-transcript\"]")
            {
                scroller.set_scroll_top(scroller.scroll_height());
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn citation_key_from_click(ev: &leptos::ev::MouseEvent) -> Option<String> {
    use wasm_bindgen::JsCast;
    let target = ev.target()?.dyn_into::<web_sys::Element>().ok()?;
    let chip = target.closest("[data-cite-key]").ok()??;
    chip.get_attribute("data-cite-key")
}

#[cfg(not(target_arch = "wasm32"))]
fn citation_key_from_click(_ev: &leptos::ev::MouseEvent) -> Option<String> {
    None
}

#[cfg(target_arch = "wasm32")]
fn scroll_cite_card(key: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let escaped = key.replace('\\', "\\\\").replace('"', "\\\"");
    let Ok(Some(card)) = document.query_selector(&format!("[data-cite-card=\"{escaped}\"]")) else {
        return;
    };
    card.scroll_into_view_with_bool(true);
}

#[cfg(not(target_arch = "wasm32"))]
fn scroll_cite_card(_key: &str) {}

/// 启动一条聊天流。浏览器 Fetch/SSE，事件进同一 reducer。
/// 任务内只允许访问 App 级 model、Router 级 navigate 与 window.location。
fn spawn_chat_stream(
    model: RwSignal<ChatCanvasModel>,
    turn: PreparedUserTurn,
    token: Option<String>,
    navigate: impl Fn(&str, NavigateOptions) + Clone + 'static,
) {
    leptos::task::spawn_local(async move {
        let is_shared = turn.request.source_type.as_deref() == Some("share");
        let client = ChatClient::new(BrowserHttpTransport::new(&poc_api_base(), token.clone()));
        let scope = turn.stream_scope;
        match client.stream(turn.request, turn.cancellation).await {
            Ok(mut stream) => {
                while let Some(item) = stream.next().await {
                    match item {
                        Ok(event) => {
                            let accepted = model.write().on_event(scope, event);
                            if accepted && !is_shared {
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
        if !is_shared { spawn_refresh_sessions(model, token, None); }
    });
}

fn spawn_refresh_sessions(
    model: RwSignal<ChatCanvasModel>,
    token: Option<String>,
    report: Option<(RwSignal<Option<String>>, RwSignal<bool>)>,
) {
    let Some(token) = token.filter(|value| !value.is_empty()) else {
        return;
    };
    if let Some((_, loading)) = report {
        loading.set(true);
    }
    leptos::task::spawn_local(async move {
        let client = BrowserRestClient::new(&poc_api_base(), Some(token));
        match client.list_sessions().await {
            Ok(list) => {
                model.update(|m| m.replace_session_list(list.sessions));
                if let Some((error, loading)) = report {
                    error.set(None);
                    loading.set(false);
                }
            }
            Err(err) => {
                if let Some((error, loading)) = report {
                    error.set(Some(crate::i18n::tf_now(
                        "chat.loadSessionFailed",
                        &[("error", &err.to_string())],
                    )));
                    loading.set(false);
                }
            }
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
        if history_load_gen.try_get_untracked() != Some(load_gen) {
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
                history_error.set(Some(crate::i18n::tf_now(
                    "chat.loadSessionFailed",
                    &[("error", &error.to_string())],
                )));
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
    let (session_id, workspace_id) = model.with_untracked(|m| {
        (
            m.manager().active.session_id.clone(),
            m.manager().active.workspace_id.clone(),
        )
    });
    let Some(session_id) = session_id else {
        return;
    };
    let target = if let Some(ws_id) = workspace_id {
        format!("/dashboard/{ws_id}?session={session_id}")
    } else {
        format!("/chat/{session_id}")
    };
    if current_path_and_query().as_deref() == Some(target.as_str()) {
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
fn current_path_and_query() -> Option<String> {
    let loc = web_sys::window()?.location();
    let path = loc.pathname().ok()?;
    let search = loc.search().ok().unwrap_or_default();
    Some(format!("{path}{search}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn current_path_and_query() -> Option<String> {
    None
}

fn spawn_check_byok(has_byok: RwSignal<bool>, token: String) {
    leptos::task::spawn_local(async move {
        let client = BrowserRestClient::new(&poc_api_base(), Some(token));
        if let Ok(resp) = client.list_provider_secrets().await {
            has_byok.set(web_sdk::has_quick_chat_byok(&resp.secrets));
        } else {
            has_byok.set(false);
        }
    });
}
