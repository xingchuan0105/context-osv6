use crate::*;
use ui::{HEADER_HEIGHT, NAV_WIDTH, RAIL_WIDTH, action, icon, muted};

impl ChatApp {
    fn collapsed(&self, window: &Window) -> bool {
        self.preferences
            .collapsed
            .unwrap_or(window.viewport_size().width < px(1200.))
    }

    pub(super) fn open_navigation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.viewport_size().width >= px(768.) {
            self.preferences.collapsed = Some(!self.collapsed(window));
            self.preferences.save();
            cx.notify();
            return;
        }
        self.navigation_sheet = true;
        let view = cx.entity();
        let close = view.clone();
        window.open_sheet_at(Placement::Left, cx, move |sheet, _, cx| {
            sheet
                .title("导航")
                .size(px(280.))
                .p_0()
                .resizable(false)
                .child(view.update(cx, |v, cx| v.render_navigation(false, true, cx)))
                .on_close({
                    let close = close.clone();
                    move |_, _, cx| {
                        close.update(cx, |v, cx| {
                            v.navigation_sheet = false;
                            cx.notify();
                        });
                    }
                })
        });
        cx.notify();
    }

    fn sync_material_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.navigation_sheet && window.viewport_size().width >= px(768.) {
            window.close_sheet(cx);
            self.navigation_sheet = false;
        }
        let needed = window.viewport_size().width < px(1200.)
            && self.knowledge.panel.is_some()
            && self.knowledge.confirm.is_none()
            && !self.show_services
            && !self.knowledge.overview;
        if self.material_sheet && !needed {
            window.close_sheet(cx);
            self.material_sheet = false;
        } else if needed && !self.material_sheet && !self.navigation_sheet {
            self.material_sheet = true;
            let view = cx.entity();
            let close = view.clone();
            window.open_sheet(cx, move |sheet, window, cx| {
                let width = (window.viewport_size().width - px(24.)).min(px(420.));
                let title = match view.read(cx).knowledge.panel {
                    Some(knowledge_view::Panel::Notes) => "工作区笔记",
                    Some(knowledge_view::Panel::Preview) => "来源原文",
                    _ => "工作区资料",
                };
                sheet
                    .title(title)
                    .size(width)
                    .p_0()
                    .resizable(false)
                    .child(view.update(cx, |v, cx| v.render_knowledge_panel(false, cx)))
                    .on_close({
                        let close = close.clone();
                        move |_, _, cx| {
                            close.update(cx, |v, cx| {
                                v.knowledge.panel = None;
                                v.material_sheet = false;
                                cx.notify();
                            });
                        }
                    })
            });
        }
    }

    pub(super) fn render_navigation(
        &self,
        collapsed: bool,
        drawer: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let workspace = self.knowledge.active.is_some();
        let mut nav = div()
            .id("sidebar")
            .test_support()
            .flex()
            .flex_col()
            .h_full()
            .min_h_0()
            .w(px(if drawer {
                280.
            } else if collapsed {
                RAIL_WIDTH
            } else {
                NAV_WIDTH
            }))
            .flex_shrink_0()
            .bg(cx.theme().sidebar)
            .text_color(cx.theme().sidebar_foreground)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .px_2()
            .py_3()
            .gap_2();
        if !drawer {
            nav = nav.child(
                div()
                    .h(px(32.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_2()
                    .when(!collapsed, |v| {
                        v.child(div().text_size(px(18.)).child("Context-OS"))
                    })
                    .child(
                        action(
                            "toggle-navigation",
                            if collapsed {
                                IconName::PanelLeftOpen
                            } else {
                                IconName::PanelLeftClose
                            },
                            "",
                        )
                        .accessibility_label(if collapsed {
                            "展开导航"
                        } else {
                            "收起导航"
                        })
                        .tooltip(if collapsed {
                            "展开导航"
                        } else {
                            "收起导航"
                        })
                        .on_click(cx.listener(|v, _, window, cx| v.open_navigation(window, cx))),
                    ),
            );
        }
        for (id, glyph, label, selected, destination) in [
            (
                "personal-nav",
                IconName::Bot,
                "个人聊天",
                !self.knowledge.overview && !workspace,
                Destination::Personal,
            ),
            (
                "workspaces-nav",
                IconName::Folder,
                "工作区",
                self.knowledge.overview || workspace,
                Destination::Overview,
            ),
        ] {
            let short = if id == "personal-nav" {
                "聊天"
            } else {
                "工作区"
            };
            nav = nav.child(
                Button::new(id)
                    .ghost()
                    .rounded(px(8.))
                    .w_full()
                    .h(px(if collapsed { 56. } else { 40. }))
                    .selected(selected)
                    .accessibility_label(label)
                    .tooltip(label)
                    .child(
                        div()
                            .flex()
                            .w_full()
                            .items_center()
                            .gap_3()
                            .when(collapsed, |v| v.flex_col().justify_center().gap_1())
                            .child(icon(glyph))
                            .child(
                                div()
                                    .text_size(px(if collapsed { 10. } else { 14. }))
                                    .child(if collapsed { short } else { label }),
                            ),
                    )
                    .on_click(cx.listener(move |v, _, window, cx| {
                        if v.navigation_sheet {
                            window.close_sheet(cx);
                            v.navigation_sheet = false;
                        }
                        v.navigate(destination.clone(), window, cx);
                    })),
            );
        }
        nav = nav.child(
            Button::new("new-chat")
                .ghost()
                .rounded(px(8.))
                .w_full()
                .h(px(if collapsed { 56. } else { 40. }))
                .accessibility_label(if workspace {
                    "工作区新对话"
                } else {
                    "新对话"
                })
                .tooltip(if workspace {
                    "工作区新对话"
                } else {
                    "新对话"
                })
                .child(
                    div()
                        .flex()
                        .w_full()
                        .items_center()
                        .gap_3()
                        .when(collapsed, |v| v.flex_col().justify_center().gap_1())
                        .child(icon(IconName::Plus))
                        .child(
                            div()
                                .text_size(px(if collapsed { 10. } else { 14. }))
                                .child(if collapsed {
                                    "新对话"
                                } else if workspace {
                                    "工作区新对话"
                                } else {
                                    "新对话"
                                }),
                        ),
                )
                .on_click(cx.listener(|v, _, window, cx| {
                    if v.navigation_sheet {
                        window.close_sheet(cx);
                        v.navigation_sheet = false;
                    }
                    v.stop();
                    v.loading = false;
                    v.conversation.reset(None);
                    v.notice.clear();
                    v.show_services = false;
                    v.knowledge.overview = false;
                    window.focus(&v.input.focus_handle(cx), cx);
                    cx.notify();
                })),
        );
        let mut history = div()
            .id("history-list")
            .test_support()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1()
            .pt_4();
        if !collapsed {
            history = history.child(
                muted(
                    if workspace {
                        "此工作区的对话"
                    } else {
                        "最近对话"
                    },
                    cx,
                )
                .px_3()
                .mb_2(),
            );
            if let Some(error) = &self.title_error {
                history = history.child(muted("部分会话名称未保存", cx).px_3()).child(
                    action("retry-titles", IconName::RotateCw, "重试")
                        .tooltip(error.clone())
                        .on_click(cx.listener(|v, _, _, _| {
                            if let Some(token) = &v.token {
                                v.host.sessions(token.clone(), v.knowledge.scope());
                            }
                        })),
                );
            }
            if self.sessions.is_empty() {
                history = history.child(
                    muted(
                        if self.token.is_none() {
                            "连接后显示本机聊天记录"
                        } else {
                            "新的对话会显示在这里"
                        },
                        cx,
                    )
                    .px_3(),
                );
            }
            for session in &self.sessions {
                let id = session.id.clone();
                let label = session_label(session);
                history = history.child(
                    Button::new(SharedString::from(id.clone()))
                        .ghost()
                        .w_full()
                        .rounded(px(8.))
                        .h(px(38.))
                        .selected(self.conversation.session_id.as_deref() == Some(id.as_str()))
                        .accessibility_label(label.clone())
                        .tooltip(label.clone())
                        .child(
                            div()
                                .w_full()
                                .min_w_0()
                                .text_size(px(13.))
                                .text_ellipsis()
                                .child(label),
                        )
                        .on_click(cx.listener(move |v, _, window, cx| {
                            if v.navigation_sheet {
                                window.close_sheet(cx);
                                v.navigation_sheet = false;
                            }
                            v.show_services = false;
                            v.stop();
                            let generation = v.conversation.reset(Some(id.clone()));
                            if let Some(token) = &v.token {
                                v.loading = true;
                                v.host.history(token.clone(), id.clone(), generation);
                            }
                            cx.notify();
                        })),
                );
            }
        }
        let dark = cx.theme().mode == ThemeMode::Dark;
        nav.child(history).child(
            div()
                .flex_shrink_0()
                .pt_3()
                .border_t_1()
                .border_color(cx.theme().sidebar_border)
                .child(
                    action(
                        "toggle-theme",
                        if dark { IconName::Sun } else { IconName::Moon },
                        if collapsed {
                            ""
                        } else if dark {
                            "浅色外观"
                        } else {
                            "深色外观"
                        },
                    )
                    .w_full()
                    .accessibility_label(if dark {
                        "切换浅色外观"
                    } else {
                        "切换深色外观"
                    })
                    .tooltip(if dark {
                        "切换浅色外观"
                    } else {
                        "切换深色外观"
                    })
                    .on_click(cx.listener(move |v, _, window, cx| {
                        ui::apply_theme(
                            if dark {
                                ThemeMode::Light
                            } else {
                                ThemeMode::Dark
                            },
                            window,
                            cx,
                        );
                        v.preferences.dark = Some(!dark);
                        v.preferences.save();
                        cx.notify();
                    })),
                )
                .when(!collapsed, |v| {
                    v.child(muted("Context-OS · 本机客户端", cx).px_3().pt_2())
                })
                .when_some(self.preferences.error.clone(), |v, error| {
                    v.child(muted(error, cx))
                }),
        )
    }

    fn render_header(&self, mobile: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let title = if self.show_services {
            "本机服务".into()
        } else if self.knowledge.overview {
            "工作区".into()
        } else {
            self.knowledge
                .active
                .as_ref()
                .map(|w| w.name.clone())
                .unwrap_or_else(|| "个人聊天".into())
        };
        let mut left = div().flex().items_center().gap_2().min_w_0().flex_1();
        if mobile {
            left = left.child(
                action("open-navigation", IconName::Menu, "")
                    .accessibility_label("打开导航")
                    .tooltip("打开导航")
                    .on_click(cx.listener(|v, _, window, cx| v.open_navigation(window, cx))),
            );
        }
        if self.show_services {
            left = left.child(
                action("back-to-chat", IconName::ArrowLeft, "返回")
                    .accessibility_label("返回聊天")
                    .disabled(self.exiting)
                    .on_click(cx.listener(|v, _, _, cx| {
                        v.show_services = false;
                        cx.notify();
                    })),
            );
        }
        left = left.child(
            div()
                .min_w_0()
                .text_ellipsis()
                .text_size(px(14.))
                .child(title),
        );
        let mut right = div().flex().items_center().gap_1().flex_shrink_0();
        if self.knowledge.active.is_some()
            && !self.show_services
            && self.knowledge.confirm.is_none()
        {
            right = right
                .child(
                    action("workspace-documents", IconName::FileText, "资料")
                        .selected(matches!(self.knowledge.panel, Some(Panel::Documents)))
                        .on_click(cx.listener(|v, _, _, cx| {
                            v.knowledge.panel = Some(Panel::Documents);
                            cx.notify();
                        })),
                )
                .child(
                    action("workspace-notes", IconName::BookOpen, "笔记")
                        .selected(matches!(self.knowledge.panel, Some(Panel::Notes)))
                        .on_click(cx.listener(|v, _, _, cx| {
                            v.knowledge.panel = Some(Panel::Notes);
                            cx.notify();
                        })),
                )
                .child(
                    action(
                        "add-workspace-document",
                        IconName::Plus,
                        if mobile { "添加" } else { "添加资料" },
                    )
                    .accessibility_label("添加工作区资料")
                    .disabled(self.token.is_none())
                    .on_click(cx.listener(|v, _, window, cx| v.pick_document(window, cx))),
                );
        }
        let mut workspace_actions = if self.knowledge.active.is_some()
            && !self.show_services
            && self.knowledge.confirm.is_none()
        {
            Some(right)
        } else {
            None
        };
        let mut right = div().flex().items_center().gap_1().flex_shrink_0();
        if !mobile {
            right = right.children(workspace_actions.take());
        }
        if !self.show_services {
            right = right.child(
                action(
                    "local-services",
                    IconName::HardDrive,
                    if mobile { "" } else { "本机服务" },
                )
                .accessibility_label("本机服务")
                .tooltip(if self.token.is_some() {
                    "本机服务 · 已连接"
                } else {
                    "本机服务 · 未连接"
                })
                .on_click(cx.listener(|v, _, _, cx| {
                    v.show_services = true;
                    if !v.services.busy() {
                        v.services.phase = Phase::Checking;
                        v.host.refresh_services();
                    }
                    cx.notify();
                })),
            );
        }
        div()
            .id("context-header")
            .test_support()
            .flex_shrink_0()
            .px_3()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(HEADER_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(left)
                    .child(right),
            )
            .children(workspace_actions.map(|actions| {
                div()
                    .h(px(44.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_end()
                    .child(actions)
            }))
    }
}

impl Render for ChatApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_material_sheet(window, cx);
        let mobile = window.viewport_size().width < px(768.);
        let wide = window.viewport_size().width >= px(1200.);
        let mut content = div()
            .id("page-content")
            .test_support()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .h_full()
            .child(self.render_header(mobile, cx));
        content = if self.knowledge.confirm.is_some() {
            content.child(self.render_confirm(cx))
        } else if self.show_services {
            content.child(self.render_services(cx))
        } else if self.knowledge.overview {
            content.child(self.render_overview(cx))
        } else {
            content.child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .flex()
                    .child(self.render_chat(window, cx))
                    .when(wide && self.knowledge.panel.is_some(), |v| {
                        v.child(self.render_knowledge_panel(true, cx))
                    }),
            )
        };
        div()
            .id("app-shell")
            .test_support()
            .flex()
            .size_full()
            .overflow_hidden()
            .text_size(px(14.))
            .line_height(relative(1.5))
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .when(!mobile, |v| {
                v.child(self.render_navigation(self.collapsed(window), false, cx))
            })
            .child(content)
    }
}
