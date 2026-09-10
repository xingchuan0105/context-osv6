use crate::*;
use ui::{READING_WIDTH, action, icon, muted};

impl ChatApp {
    pub(super) fn render_chat(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let streaming = self.conversation.turn.status == TurnStatus::Streaming;
        let empty = self.conversation.messages.is_empty()
            && self.conversation.turn.answer_text.is_empty()
            && !self.loading;
        let compact = window.viewport_size().width < px(768.);
        let workspace = self.knowledge.active.is_some();
        let status = match &self.conversation.turn.status {
            TurnStatus::Streaming => "正在回答…".to_owned(),
            TurnStatus::Cancelled => "已停止，已显示的内容已保留".into(),
            TurnStatus::Error { message, .. } => message.clone(),
            TurnStatus::Done => String::new(),
            TurnStatus::Idle if self.loading => "正在恢复会话…".into(),
            TurnStatus::Idle if self.token.is_none() => self.notice.clone(),
            TurnStatus::Idle => String::new(),
        };
        let error = matches!(self.conversation.turn.status, TurnStatus::Error { .. });
        let mut messages = div()
            .id("messages")
            .test_support()
            .track_scroll(&self.scroll)
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .items_center()
            .gap_6()
            .px(px(if compact { 16. } else { 28. }))
            .py_6();
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
                cx,
            ));
            if let Some(citations) = self.conversation.citations.get(&index) {
                messages =
                    messages.child(self.citation_row(&format!("history-{index}"), citations, cx));
            }
        }
        if !self.conversation.turn.answer_text.is_empty() {
            messages = messages.child(render_message(
                format!("message-{session_key}-{}", self.conversation.messages.len()),
                "assistant",
                &self.conversation.turn.answer_text,
                cx,
            ));
            let citations = self
                .conversation
                .turn
                .citations
                .iter()
                .map(web_sdk::CitationView::from_value)
                .collect::<Vec<_>>();
            messages = messages.child(self.citation_row("current", &citations, cx));
        }
        if self.loading {
            messages = messages.child(muted("正在恢复会话…", cx));
        }
        if empty {
            messages = messages.justify_end().gap_0().pb_3().child(
                div()
                    .id("chat-welcome")
                    .test_support()
                    .w_full()
                    .max_w(px(READING_WIDTH))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_color(cx.theme().muted_foreground)
                            .child(icon(if workspace {
                                IconName::Folder
                            } else {
                                IconName::Bot
                            }))
                            .child(div().text_size(px(13.)).child(if workspace {
                                "工作区对话"
                            } else {
                                "Context-OS"
                            })),
                    )
                    .child(
                        div()
                            .text_size(px(if compact { 26. } else { 32. }))
                            .line_height(relative(1.25))
                            .child(if workspace {
                                "让资料成为思考的起点"
                            } else {
                                "今天想聊些什么？"
                            }),
                    )
                    .child(muted(
                        if workspace {
                            "添加资料、提出问题，也可以随时把想法记入笔记。"
                        } else if self.token.is_some() {
                            "从一个问题开始，或者继续之前的对话。"
                        } else {
                            "连接本机服务，开始对话。"
                        },
                        cx,
                    )),
            );
        }
        let context = if workspace {
            if self.knowledge.selected.is_empty() {
                "工作区 · 全部可检索资料".to_owned()
            } else {
                format!("工作区 · {} 份已选资料", self.knowledge.selected.len())
            }
        } else {
            "个人聊天".into()
        };
        let mut toolbar = div()
            .id("composer-toolbar")
            .test_support()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .min_w_0()
                    .child(icon(if workspace {
                        IconName::Folder
                    } else {
                        IconName::Bot
                    }))
                    .child(muted(context, cx).text_ellipsis()),
            );
        if self.token.is_none() {
            toolbar = toolbar.child(
                Button::new("connect")
                    .primary()
                    .rounded(px(10.))
                    .icon(icon(IconName::Play))
                    .label(if self.connecting {
                        "连接中…"
                    } else if self.connection_attempted {
                        "重试连接"
                    } else {
                        "连接服务"
                    })
                    .text_size(px(13.))
                    .disabled(self.services.busy())
                    .on_click(cx.listener(|v, _, _, cx| v.connect(cx))),
            );
        } else if streaming {
            toolbar = toolbar.child(
                Button::new("stop")
                    .rounded(px(10.))
                    .icon(icon(IconName::Pause))
                    .label("停止")
                    .on_click(cx.listener(|v, _, _, cx| {
                        v.stop();
                        cx.notify();
                    })),
            );
        } else {
            toolbar = toolbar.child(
                Button::new("send")
                    .primary()
                    .rounded(px(10.))
                    .icon(icon(IconName::ArrowUp))
                    .label("发送")
                    .disabled(self.loading || self.services.busy())
                    .on_click(cx.listener(|v, _, window, cx| v.send(window, cx))),
            );
        }
        let composer = div()
            .id("composer")
            .test_support()
            .w_full()
            .max_w(px(READING_WIDTH))
            .border_1()
            .border_color(cx.theme().border)
            .rounded(px(16.))
            .bg(cx.theme().background)
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .id("composer-input")
                    .test_support()
                    .track_focus(&self.input.focus_handle(cx))
                    .child(
                        Textarea::new(&self.input)
                            .appearance(false)
                            .bordered(false)
                            .aria_label(if workspace {
                                "向工作区提问"
                            } else {
                                "输入聊天问题"
                            })
                            .h(px(if compact { 64. } else { 80. }))
                            .text_size(px(15.)),
                    ),
            )
            .child(toolbar);
        div()
            .id("chat-canvas")
            .test_support()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .h_full()
            .child(messages)
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .px(px(if compact { 16. } else { 28. }))
                    .pb_4()
                    .when(!status.is_empty(), |v| {
                        v.child(
                            div()
                                .id("turn-status")
                                .test_support()
                                .w_full()
                                .max_w(px(READING_WIDTH))
                                .rounded_lg()
                                .when(error, |v| {
                                    v.p_3()
                                        .bg(cx.theme().muted)
                                        .border_1()
                                        .border_color(cx.theme().danger)
                                })
                                .child(muted(status, cx)),
                        )
                    })
                    .child(composer)
                    .when(workspace && empty, |v| {
                        v.child(muted("笔记单独保存，不会自动加入问答资料。", cx))
                    }),
            )
            .when(empty, |v| v.child(div().flex_1().min_h(px(16.))))
    }
}

fn render_message(id: String, role: &str, text: &str, cx: &App) -> Div {
    let mut row = div()
        .w_full()
        .min_w_0()
        .max_w(px(READING_WIDTH))
        .flex()
        .flex_col()
        .gap_3();
    if role == "assistant" {
        let mut table = StyleRefinement::default();
        table.overflow.x = Some(Overflow::Scroll);
        let copy = text.to_owned();
        row = row
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_color(cx.theme().muted_foreground)
                    .child(icon(IconName::Bot))
                    .child(div().text_size(px(12.)).child("Context-OS")),
            )
            .child(
                TextView::markdown(SharedString::from(id.clone()), text.to_owned())
                    .plugin(markdown_view::ChatMarkdown::new(id.clone()))
                    .w_full()
                    .min_w_0()
                    .text_size(px(15.))
                    .line_height(relative(1.7))
                    .style(TextViewStyle::default().table(table)),
            )
            .child(
                div().flex().child(
                    action(SharedString::from(format!("copy-{id}")), IconName::Copy, "")
                        .accessibility_label("复制回复")
                        .tooltip("复制回复")
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))
                        }),
                ),
            );
    } else {
        row = row.items_end().child(
            div()
                .max_w_full()
                .min_w_0()
                .px_4()
                .py_3()
                .rounded(px(16.))
                .bg(cx.theme().muted)
                .text_size(px(15.))
                .line_height(relative(1.65))
                .child(text.to_owned()),
        );
    }
    row
}
