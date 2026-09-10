use crate::*;
use ui::{action, badge, card, icon, muted};

impl ChatApp {
    pub(super) fn render_services(&self, cx: &mut Context<Self>) -> Div {
        let busy = self.services.busy();
        let mut body = div().flex().flex_col().gap_4().w_full().max_w(px(880.));
        body = body
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .mb_3()
                    .child(div().text_size(px(28.)).child("让本机随时准备好"))
                    .child(muted("查看连接与处理状态，管理此客户端启动的服务。", cx)),
            )
            .child(
                card(cx)
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(icon(IconName::HardDrive))
                    .child(
                        div()
                            .flex_1()
                            .child("本机运行状态")
                            .child(muted(self.services.phase.label(), cx)),
                    )
                    .child(badge(
                        if self.token.is_some() {
                            "已连接"
                        } else if busy {
                            "准备中"
                        } else {
                            "未连接"
                        },
                        cx,
                    )),
            );
        if let Some(error) = &self.services.error {
            body = body.child(
                card(cx)
                    .border_color(cx.theme().danger)
                    .child(error.clone())
                    .child(
                        div()
                            .text_sm()
                            .child("可重新连接或打开日志查看详情，输入框草稿会保留。"),
                    ),
            );
        }
        if let Some(snapshot) = &self.services.snapshot {
            body = body.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(if snapshot.attached {
                        "正在连接独立启动的本机服务；退出客户端会保留这些服务。"
                    } else if snapshot.owned {
                        "退出时会停止本次由客户端启动的服务，保留原有服务。"
                    } else {
                        "使用本机数据。启动时会准备数据库、执行迁移并建立个人会话。"
                    }),
            );
            body = body.child(service_row(
                "聊天服务",
                if snapshot.product.api_ok {
                    "健康检查通过"
                } else {
                    "未就绪"
                },
                &snapshot.product.api_base_url,
                cx,
            ));
            if !snapshot.product.api_ok {
                body = body.child(
                    div()
                        .text_sm()
                        .child(snapshot.product.health_detail.clone()),
                );
            }
            if let Some(stack) = &snapshot.stack {
                for service in &stack.services {
                    body = body.child(service_row(
                        &service.label,
                        if service.ok {
                            "端口可达"
                        } else {
                            "端口不可达"
                        },
                        &service.endpoint,
                        cx,
                    ));
                }
                body = body
                    .child(service_row(
                        "后台任务",
                        if snapshot.product.worker_ok {
                            "进程运行中"
                        } else {
                            "未运行"
                        },
                        &snapshot.product.worker_detail,
                        cx,
                    ))
                    .child(service_row(
                        "本机运行工具",
                        if snapshot.tools_available {
                            "已找到"
                        } else {
                            "缺少运行文件"
                        },
                        "PostgreSQL / Redis；无需 Docker",
                        cx,
                    ))
                    .child(service_row(
                        "运行配置",
                        if snapshot.env_exists {
                            "已生成"
                        } else {
                            "启动时生成"
                        },
                        "迁移成功后才建立本机会话",
                        cx,
                    ));
            } else {
                body = body.child(div().text_sm().child(
                    "独立服务的数据库、缓存与后台任务由其启动环境管理，此处仅检查聊天 API。",
                ));
            }
            body = body.child(service_row(
                "本机会话",
                if self.token.is_some() {
                    "已连接"
                } else {
                    "未连接"
                },
                "无需云端登录",
                cx,
            ));
            let logs = snapshot.logs.clone();
            body = body.child(
                action("open-service-logs", IconName::FolderOpen, "")
                    .label(if snapshot.attached {
                        "打开服务运行目录"
                    } else {
                        "打开日志目录"
                    })
                    .disabled(!logs.is_dir())
                    .on_click(move |_, _, cx| cx.reveal_path(&logs)),
            );
        }
        let mut actions = div().flex().flex_wrap().gap_2().child(
            action("refresh-services", IconName::RotateCw, "刷新状态")
                .disabled(busy || self.exiting)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.services.phase = Phase::Checking;
                    this.services.error = None;
                    this.host.refresh_services();
                    cx.notify();
                })),
        );
        if self.token.is_none()
            || self
                .services
                .snapshot
                .as_ref()
                .is_some_and(|s| !s.attached && !s.product.overall_ok)
        {
            actions = actions.child(
                Button::new("start-services")
                    .primary()
                    .label(
                        if self.services.snapshot.as_ref().is_some_and(|s| s.attached) {
                            "重新连接"
                        } else {
                            "启动并连接"
                        },
                    )
                    .disabled(
                        busy || self.exiting
                            || self.conversation.turn.status == TurnStatus::Streaming,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.connect(cx))),
            );
        }
        if self.token.is_some() || self.services.snapshot.as_ref().is_some_and(|s| s.owned) {
            actions = actions.child(
                Button::new("stop-services")
                    .label(
                        if self.services.snapshot.as_ref().is_some_and(|s| s.owned) {
                            "停止本次启动的服务"
                        } else {
                            "断开会话"
                        },
                    )
                    .disabled(busy || self.exiting)
                    .on_click(cx.listener(|this, _, _, cx| this.stop_services(cx))),
            );
        }
        body = body.child(card(cx).child(actions));
        div().flex().flex_col().flex_1().min_w_0().min_h_0().child(
            div()
                .id("service-content")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .px_4()
                .py_6()
                .flex()
                .flex_col()
                .items_center()
                .child(body)
                .test_support(),
        )
    }
}

fn service_row(title: &str, status: &str, detail: &str, cx: &App) -> Div {
    card(cx)
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .flex()
                .flex_wrap()
                .justify_between()
                .gap_2()
                .child(title.to_owned())
                .child(badge(status.to_owned(), cx)),
        )
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(detail.to_owned()),
        )
}
