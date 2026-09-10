use crate::*;

impl ChatApp {
    pub(super) fn render_services(&self, cx: &mut Context<Self>) -> Div {
        let busy = self.services.busy();
        let mut body = div().flex().flex_col().gap_5().w_full().max_w(px(760.));
        body = body.child(div().text_lg().child(self.services.phase.label()));
        if let Some(error) = &self.services.error {
            body = body.child(
                div()
                    .p_3()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
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
                Button::new("open-service-logs")
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
            Button::new("refresh-services")
                .label("刷新状态")
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
        body = body.child(actions);
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
                    .gap_3()
                    .child(
                        Button::new("back-to-chat")
                            .ghost()
                            .label("返回聊天")
                            .disabled(self.exiting)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_services = false;
                                cx.notify();
                            })),
                    )
                    .child("本机服务"),
            )
            .child(
                div()
                    .id("service-content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_6()
                    .child(body),
            )
    }
}

fn service_row(title: &str, status: &str, detail: &str, cx: &App) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .pb_3()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .flex()
                .flex_wrap()
                .justify_between()
                .gap_2()
                .child(title.to_owned())
                .child(status.to_owned()),
        )
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(detail.to_owned()),
        )
}
