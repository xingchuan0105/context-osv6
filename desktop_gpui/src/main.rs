use contracts::workspaces::ChatSession;
use desktop_gpui::{
    runtime::{Host, Update},
    session::Conversation,
    session_titles::session_label,
};
use futures::StreamExt;
use gpui_kit::prelude::FluentBuilder;
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
}

impl ChatApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (host, mut updates) = Host::new().expect("create desktop runtime");
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(3)
                .placeholder("输入问题…")
        });
        cx.spawn(async move |this, cx| {
            while let Some(update) = updates.next().await {
                if this
                    .update(cx, |this, cx| {
                        this.apply(update);
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
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
        }
    }

    fn apply(&mut self, update: Update) {
        match update {
            Update::Login(result) => {
                self.connecting = false;
                match result {
                    Ok(session) => {
                        self.token = session.token;
                        if let Some(token) = &self.token {
                            self.notice = "本地会话已连接".into();
                            self.host.sessions(token.clone());
                        } else {
                            self.notice = "本地会话未返回凭据，请重试".into();
                        }
                    }
                    Err(error) => self.notice = error,
                }
            }
            Update::Sessions(result) => match result {
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
                        self.conversation.messages = messages
                            .into_iter()
                            .filter(|m| m.role == "user" || m.role == "assistant")
                            .map(|m| (m.role, m.content))
                            .collect();
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
                    self.host.sessions(token.clone());
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
        if self.loading {
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
            generation,
        ));
        self.scroll.scroll_to_bottom();
        cx.notify();
    }
}

impl Drop for ChatApp {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Render for ChatApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            .w(px(248.))
            .flex_shrink_0()
            .h_full()
            .p_4()
            .gap_3()
            .border_r_1()
            .border_color(cx.theme().border)
            .child(div().text_lg().child("Context-OS"))
            .child(
                Button::new("new-chat")
                    .label("新对话")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.stop();
                        this.loading = false;
                        this.conversation.reset(None);
                        this.notice.clear();
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("个人聊天 · 历史会话"),
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
                                this.host.sessions(token.clone());
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
        }
        if !self.conversation.turn.answer_text.is_empty() {
            messages = messages.child(render_message(
                format!("message-{session_key}-{}", self.conversation.messages.len()),
                "assistant",
                &self.conversation.turn.answer_text,
            ));
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
                    .child(div().text_2xl().mb_3().child("今天想聊些什么？"))
                    .child(div().text_color(cx.theme().muted_foreground).child(
                        if self.token.is_some() {
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
                    .disabled(self.connecting)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(path) = std::env::var_os("CONTEXT_OS_DESKTOP_DATA_DIR")
                            .map(std::path::PathBuf::from)
                            .or_else(|| dirs::data_dir().map(|p| p.join("com.contextos.desktop")))
                        {
                            this.connecting = true;
                            this.connection_attempted = true;
                            this.notice =
                                "正在准备本机服务并恢复会话，首次启动可能需要一些时间。".into();
                            this.host.login(path);
                        } else {
                            this.notice = "无法确定本地数据目录".into();
                        }
                        cx.notify();
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
                    .disabled(self.loading)
                    .on_click(cx.listener(|this, _, window, cx| this.send(window, cx))),
            );
        }
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
                            .h(px(52.))
                            .flex_shrink_0()
                            .px_4()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child("个人聊天")
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(if self.connecting {
                                        "正在连接"
                                    } else if self.token.is_some() {
                                        "本机 · 已连接"
                                    } else {
                                        "本机 · 未连接"
                                    }),
                            ),
                    )
                    .child(messages)
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
                                    .child(Textarea::new(&self.input).h(px(112.)))
                                    .child(controls),
                            ),
                    )
                    .when(empty, |content| content.child(div().flex_1())),
            )
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
            TextView::markdown(SharedString::from(id), text.to_owned())
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
