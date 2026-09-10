use contracts::workspaces::ChatSession;
use desktop_gpui::{
    runtime::{Host, Update},
    services::{Phase, ServiceViewState},
    session::Conversation,
    session_titles::session_label,
};
use futures::StreamExt;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::base::TestSupportExt;
use gpui_kit::{
    component::{
        button::*,
        input::{Textarea, TextareaState},
        text::{TextView, TextViewStyle},
        *,
    },
    *,
};
use tokio_util::sync::CancellationToken;
use web_sdk::TurnStatus;

mod markdown_view;
mod service_view;
mod knowledge_view;
mod knowledge_render;
use knowledge_view::{Knowledge, Destination, Panel};
use desktop_gpui::workspace::Action as KnowledgeAction;
#[cfg(all(test, feature = "headless-tests"))]
mod ui_tests;

struct ChatApp {
    host: Host,
    input: Entity<TextareaState>,
    conversation: Conversation,
    sessions: Vec<ChatSession>,
    token: Option<String>,
    connecting: bool,
    connection_attempted: bool,
    loading: bool,
    notice: String,
    title_error: Option<String>,
    cancel: Option<CancellationToken>,
    scroll: ScrollHandle,
    services: ServiceViewState,
    show_services: bool,
    exiting: bool,
    exit_ready: bool,
    knowledge: Knowledge,
}

impl ChatApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (host, mut updates) = Host::new().expect("create desktop runtime");
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(3)
                .placeholder("输入问题…")
        });
        let knowledge = Knowledge::new(window, cx);
        cx.spawn_in(window, async move |this, cx| {
            while let Some(update) = updates.next().await {
                if this
                    .update_in(cx, |this, window, cx| {
                        this.apply(update, window, cx);
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_secs(3)).await;
                if this.update(cx, |this, cx| this.poll_documents(cx)).is_err() { break; }
            }
        }).detach();
        let weak = cx.entity().downgrade();
        window.on_window_should_close(cx, move |_, cx| {
            weak.update(cx, |this, cx| {
                if this.exit_ready {
                    return true;
                }
                this.request_exit(cx);
                false
            })
            .unwrap_or(true)
        });
        host.refresh_services();
        Self {
            host,
            input,
            conversation: Conversation::default(),
            sessions: vec![],
            token: None,
            connecting: false,
            connection_attempted: false,
            loading: false,
            notice: "连接本机服务后开始聊天".into(),
            title_error: None,
            cancel: None,
            scroll: ScrollHandle::new(),
            services: ServiceViewState {
                phase: Phase::Checking,
                ..Default::default()
            },
            show_services: false,
            exiting: false,
            exit_ready: false,
            knowledge,
        }
    }

    fn apply(&mut self, update: Update, window: &mut Window, cx: &mut Context<Self>) {
        match update {
            Update::ServicePhase(phase) => {
                if !self.exiting && self.services.phase != Phase::Stopping {
                    self.services.phase = phase;
                }
            }
            Update::ServiceSnapshot(result) => {
                match result {
                    Ok(snapshot) => {
                        if !snapshot.product.api_ok && self.token.is_some() {
                            self.stop();
                            self.token = None;
                            self.notice =
                                "本机服务已断开，请重新连接；草稿和已显示内容保留。".into();
                        }
                        self.services.snapshot = Some(snapshot);
                    }
                    Err(error) => self.services.error = Some(error),
                }
                if !self.connecting && !self.exiting && self.services.phase != Phase::Stopping {
                    self.services.phase = if self.token.is_some() {
                        Phase::Ready
                    } else {
                        Phase::Idle
                    };
                }
            }
            Update::ServicesStopped(result) => {
                self.connecting = false;
                match result {
                    Ok(()) => {
                        self.token = None;
                        self.services.phase = Phase::Stopped;
                        self.services.error = None;
                        self.notice = "本机会话已断开，已有正文和草稿保留。".into();
                        if self.exiting {
                            self.exit_ready = true;
                            cx.quit();
                        }
                    }
                    Err(error) => {
                        self.exiting = false;
                        self.services.phase = Phase::Failed;
                        self.services.error = Some(error);
                    }
                }
            }
            Update::Login(result) => {
                self.connecting = false;
                if self.exiting || self.services.phase == Phase::Stopping {
                    return;
                }
                match result {
                    Ok(session) => {
                        self.token = session.token;
                        if let Some(token) = &self.token {
                            self.services.phase = Phase::Ready;
                            self.services.error = None;
                            self.notice = "本地会话已连接".into();
                            self.host.sessions(token.clone(), self.knowledge.scope());
                            if self.knowledge.overview { self.knowledge_action(KnowledgeAction::List, cx); }
                            if self.knowledge.active.is_some() { self.knowledge_action(KnowledgeAction::Load, cx); }
                        } else {
                            self.notice = "本地会话未返回凭据，请重试".into();
                            self.services.phase = Phase::Failed;
                            self.services.error = Some(self.notice.clone());
                        }
                    }
                    Err(error) => {
                        self.services.phase = Phase::Failed;
                        self.services.error = Some(error.clone());
                        self.notice = error;
                    }
                }
            }
            Update::Workspace(reply) => self.apply_knowledge(reply, window, cx),
            Update::Sessions(scope, result) if scope == self.knowledge.scope() => match result {
                Ok(sessions) => {
                    self.sessions = sessions;
                    self.title_error = None;
                }
                Err(error) => self.notice = format!("会话列表加载失败：{error}"),
            },
            Update::SessionTitle(result) => match result {
                Ok(session) => {
                    if let Some(row) = self.sessions.iter_mut().find(|row| row.id == session.id) {
                        *row = session;
                    }
                }
                Err(error) => self.title_error = Some(error),
            },
            Update::History(generation, result) if generation == self.conversation.generation => {
                self.loading = false;
                match result {
                    Ok(messages) => {
                        self.conversation.restore(messages);
                        self.notice = "历史已恢复".into();
                    }
                    Err(error) => self.notice = format!("历史加载失败：{error}；点击会话重试"),
                }
            }
            Update::Event(generation, event) => {
                let follow = self.scroll.max_offset().y + self.scroll.offset().y <= px(24.);
                self.conversation.event(generation, event);
                if follow {
                    self.scroll.scroll_to_bottom();
                }
            }
            Update::End(generation, result) if generation == self.conversation.generation => {
                self.cancel = None;
                if self.conversation.turn.status == TurnStatus::Streaming {
                    self.conversation.turn.status = TurnStatus::Error {
                        code: "stream_incomplete".into(),
                        message: result
                            .err()
                            .unwrap_or_else(|| "连接结束但回答未完成，请重试".into()),
                    };
                }
                if let Some(token) = &self.token {
                    self.host.sessions(token.clone(), self.knowledge.scope());
                }
            }
            _ => {}
        }
    }

    fn stop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel.cancel();
        }
        self.conversation.cancel();
    }

    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading || self.services.busy() || self.exiting {
            return;
        }
        let Some(token) = self.token.clone() else {
            return;
        };
        let query = self.input.read(cx).value().to_string();
        let Some(generation) = self.conversation.begin(query.clone()) else {
            return;
        };
        self.input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.cancel = Some(self.host.chat(
            token,
            query,
            self.conversation.session_id.clone(),
            self.knowledge.scope(),
            self.knowledge.selected.iter().cloned().collect(),
            generation,
        ));
        self.scroll.scroll_to_bottom();
        cx.notify();
    }

    fn connect(&mut self, cx: &mut Context<Self>) {
        if self.services.busy() || self.exiting {
            return;
        }
        if let Some(path) = std::env::var_os("CONTEXT_OS_DESKTOP_DATA_DIR")
            .map(std::path::PathBuf::from)
            .or_else(|| dirs::data_dir().map(|p| p.join("com.contextos.desktop")))
        {
            self.connecting = true;
            self.connection_attempted = true;
            self.services.phase = Phase::Checking;
            self.services.error = None;
            self.notice = "正在准备本机服务并恢复会话…".into();
            self.host.login(path);
        } else {
            self.notice = "无法确定本地数据目录".into();
        }
        cx.notify();
    }

    fn stop_services(&mut self, cx: &mut Context<Self>) {
        self.stop();
        self.loading = false;
        self.conversation.generation += 1;
        self.services.phase = Phase::Stopping;
        self.host.stop_services();
        cx.notify();
    }

    fn request_exit(&mut self, cx: &mut Context<Self>) {
        if self.knowledge.dirty(cx) {
            self.knowledge.confirm = Some(knowledge_view::Confirm::Leave(Destination::Exit));
            cx.notify();
            return;
        }
        if self.exiting {
            return;
        }
        self.exiting = true;
        self.show_services = true;
        self.stop_services(cx);
    }
}

impl Drop for ChatApp {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Render for ChatApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let wide = window.viewport_size().width >= px(1100.);
        let streaming = self.conversation.turn.status == TurnStatus::Streaming;
        let empty = self.conversation.messages.is_empty()
            && self.conversation.turn.answer_text.is_empty()
            && !self.loading;
        let status = match &self.conversation.turn.status {
            TurnStatus::Streaming => "正在回答…".to_string(),
            TurnStatus::Done => "已完成".into(),
            TurnStatus::Cancelled => "已停止".into(),
            TurnStatus::Error { message, .. } => message.clone(),
            TurnStatus::Idle => self.notice.clone(),
        };
        let mut sidebar = div()
            .id("sidebar")
            .flex()
            .flex_col()
            .w(px(if window.viewport_size().width < px(900.) { 148. } else { 248. }))
            .flex_shrink_0()
            .h_full()
            .p_4()
            .gap_3()
            .border_r_1()
            .border_color(cx.theme().border)
            .child(div().text_lg().child("Context-OS"))
            .child(Button::new("personal-nav").ghost().label("个人聊天")
                .selected(!self.knowledge.overview && self.knowledge.active.is_none())
                .on_click(cx.listener(|this, _, window, cx| this.navigate(Destination::Personal, window, cx))))
            .child(Button::new("workspaces-nav").ghost().label("工作区")
                .selected(self.knowledge.overview || self.knowledge.active.is_some())
                .on_click(cx.listener(|this, _, window, cx| this.navigate(Destination::Overview, window, cx))))
            .child(
                Button::new("new-chat")
                    .label(if self.knowledge.active.is_some() { "工作区新对话" } else { "新对话" })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.stop();
                        this.loading = false;
                        this.conversation.reset(None);
                        this.notice.clear();
                        this.show_services = false;
                        this.knowledge.overview = false;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(if self.knowledge.active.is_some() { "工作区 · 历史会话" } else { "个人聊天 · 历史会话" }),
            );
        if let Some(error) = &self.title_error {
            sidebar = sidebar.child(
                div().text_sm().child("部分会话名称未保存").child(
                    Button::new("retry-titles")
                        .ghost()
                        .label("重试")
                        .tooltip(error.clone())
                        .on_click(cx.listener(|this, _, _, _| {
                            if let Some(token) = &this.token {
                                this.host.sessions(token.clone(), this.knowledge.scope());
                            }
                        })),
                ),
            );
        }
        if self.sessions.is_empty() {
            sidebar = sidebar.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(if self.token.is_none() {
                        "连接后显示本机的聊天记录"
                    } else {
                        "还没有历史会话"
                    }),
            );
        }
        for session in &self.sessions {
            let id = session.id.clone();
            let label = session_label(session);
            sidebar = sidebar.child(
                Button::new(SharedString::from(id.clone()))
                    .ghost()
                    .w_full()
                    .selected(self.conversation.session_id.as_deref() == Some(id.as_str()))
                    .accessibility_label(label.clone())
                    .tooltip(label.clone())
                    .child(div().w_full().min_w_0().text_ellipsis().child(label))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.show_services = false;
                        this.stop();
                        let generation = this.conversation.reset(Some(id.clone()));
                        if let Some(token) = &this.token {
                            this.loading = true;
                            this.host.history(token.clone(), id.clone(), generation);
                        }
                        cx.notify();
                    })),
            );
        }
        if self.knowledge.confirm.is_some() {
            return div().flex().size_full().bg(cx.theme().background).text_color(cx.theme().foreground)
                .child(sidebar.overflow_y_scroll()).child(self.render_confirm(cx));
        }
        if self.show_services {
            return div()
                .flex()
                .size_full()
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                .child(sidebar.overflow_y_scroll())
                .child(self.render_services(cx));
        }
        if self.knowledge.overview {
            return div().flex().size_full().bg(cx.theme().background).text_color(cx.theme().foreground)
                .child(sidebar.overflow_y_scroll()).child(self.render_overview(cx));
        }
        if !wide && self.knowledge.panel.is_some() {
            return div().flex().size_full().bg(cx.theme().background).text_color(cx.theme().foreground)
                .child(sidebar.overflow_y_scroll()).child(self.render_knowledge_panel(false, cx));
        }
        let mut messages = div()
            .id("messages")
            .track_scroll(&self.scroll)
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .items_center()
            .gap_4()
            .p_4();
        let session_key = self
            .conversation
            .session_id
            .clone()
            .unwrap_or_else(|| format!("draft-{}", self.conversation.generation));
        for (index, (role, text)) in self.conversation.messages.iter().enumerate() {
            messages = messages.child(render_message(
                format!("message-{session_key}-{index}"),
                role,
                text,
            ));
            if let Some(citations) = self.conversation.citations.get(&index) {
                messages = messages.child(self.citation_row(&format!("history-{index}"), citations, cx));
            }
        }
        if !self.conversation.turn.answer_text.is_empty() {
            messages = messages.child(render_message(
                format!("message-{session_key}-{}", self.conversation.messages.len()),
                "assistant",
                &self.conversation.turn.answer_text,
            ));
            let citations = self.conversation.turn.citations.iter().map(web_sdk::CitationView::from_value).collect::<Vec<_>>();
            messages = messages.child(self.citation_row("current", &citations, cx));
        }
        if self.loading {
            messages = messages.child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child("正在恢复会话…"),
            );
        }
        if empty {
            messages = messages.justify_end().child(
                div()
                    .w_full()
                    .max_w(px(760.))
                    .pb_6()
                    .child(div().text_2xl().mb_3().child(if self.knowledge.active.is_some() { "围绕资料开始提问" } else { "今天想聊些什么？" }))
                    .child(div().text_color(cx.theme().muted_foreground).child(
                        if self.knowledge.active.is_some() {
                            "添加资料后在此提问。笔记单独保存，不会自动加入检索资料。"
                        } else if self.token.is_some() {
                            "提一个问题，或继续左侧的历史对话。"
                        } else {
                            "连接本机服务后，即可开始聊天并恢复历史记录。"
                        },
                    )),
            );
        }
        let mut controls = div().flex().justify_end().gap_2();
        if self.token.is_none() {
            controls = controls.child(
                Button::new("connect")
                    .label(if self.connecting {
                        "正在准备本机服务…"
                    } else if self.connection_attempted {
                        "重试连接"
                    } else {
                        "连接本机服务"
                    })
                    .disabled(self.services.busy())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.connect(cx);
                    })),
            );
        } else if streaming {
            controls = controls.child(Button::new("stop").label("停止").on_click(cx.listener(
                |this, _, _, cx| {
                    this.stop();
                    cx.notify();
                },
            )));
        } else {
            controls = controls.child(
                Button::new("send")
                    .primary()
                    .label("发送")
                    .disabled(self.loading || self.services.busy())
                    .on_click(cx.listener(|this, _, window, cx| this.send(window, cx))),
            );
        }
        let panel = if wide && self.knowledge.panel.is_some() { Some(self.render_knowledge_panel(true, cx)) } else { None };
        div()
            .flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(sidebar.overflow_y_scroll())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(
                        div()
                            .min_h(px(52.))
                            .flex_shrink_0()
                            .px_4()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .items_center()
                            .justify_between()
                            .child(div().min_w_0().text_ellipsis().child(self.knowledge.active.as_ref().map(|w| w.name.clone()).unwrap_or_else(|| "个人聊天".into())))
                            .when(self.knowledge.active.is_some(), |header| header.child(div().flex().flex_wrap().gap_1()
                                .child(Button::new("workspace-documents").ghost().label("资料").on_click(cx.listener(|this, _, _, cx| { this.knowledge.panel = Some(Panel::Documents); cx.notify(); })))
                                .child(Button::new("workspace-notes").ghost().label("笔记").on_click(cx.listener(|this, _, _, cx| { this.knowledge.panel = Some(Panel::Notes); cx.notify(); })))
                                .child(Button::new("add-workspace-document").ghost().label("添加资料").disabled(self.token.is_none()).on_click(cx.listener(|this, _, window, cx| this.pick_document(window, cx))))))
                            .child(
                                Button::new("local-services")
                                    .ghost()
                                    .label("本机服务")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.show_services = true;
                                        if !this.services.busy() {
                                            this.services.phase = Phase::Checking;
                                            this.host.refresh_services();
                                        }
                                        cx.notify();
                                    }))
                                    .child(if self.connecting {
                                        "正在连接"
                                    } else if self.token.is_some() {
                                        "本机 · 已连接"
                                    } else {
                                        "本机 · 未连接"
                                    }),
                            ),
                    )
                    .child(messages.test_support())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_shrink_0()
                            .items_center()
                            .gap_2()
                            .p_4()
                            .child(
                                div()
                                    .w_full()
                                    .max_w(px(760.))
                                    .flex()
                                    .flex_col()
                                    .gap_3()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(status),
                                    )
                                    .child(
                                        div()
                                            .id("composer-input")
                                            .test_support()
                                            .track_focus(&self.input.focus_handle(cx))
                                            .child(Textarea::new(&self.input).h(px(112.))),
                                    )
                                    .child(controls),
                            ),
                    )
                    .when(empty, |content| content.child(div().flex_1())),
            )
            .children(panel)
    }
}

fn render_message(id: String, role: &str, text: &str) -> Div {
    let mut row = div()
        .w_full()
        .min_w_0()
        .max_w(px(760.))
        .flex()
        .flex_col()
        .gap_2()
        .child(if role == "user" { "你" } else { "Context-OS" });
    if role == "assistant" {
        let mut table = StyleRefinement::default();
        table.overflow.x = Some(Overflow::Scroll);
        row = row.child(
            TextView::markdown(SharedString::from(id.clone()), text.to_owned())
                .plugin(markdown_view::ChatMarkdown::new(id))
                .w_full()
                .min_w_0()
                .style(TextViewStyle::default().table(table)),
        );
    } else {
        row = row.child(div().child(text.to_owned()));
    }
    row
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            cx.spawn(async move |cx| {
                cx.open_window(WindowOptions::default(), |window, cx| {
                    let view = cx.new(|cx| ChatApp::new(window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                })
                .expect("open desktop window");
            })
            .detach();
        });
}
