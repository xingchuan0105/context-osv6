use contracts::workspaces::ChatSession;
use desktop_gpui::{
    runtime::{Host, Update},
    services::{Phase, ServiceViewState},
    session::Conversation,
    session_titles::session_label,
};
use futures::StreamExt;
use gpui_kit::base::TestSupportExt;
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

mod chat_view;
mod knowledge_render;
mod knowledge_view;
mod markdown_view;
mod service_view;
mod shell_view;
mod ui;
#[cfg(all(feature = "headless-tests", target_os = "windows"))]
mod visual_preview;
use desktop_gpui::workspace::Action as KnowledgeAction;
use knowledge_view::{Destination, Knowledge, Panel};
#[cfg(all(test, feature = "headless-tests"))]
mod ui_tests;

struct ChatApp {
    preferences: ui::Preferences,
    navigation_sheet: bool,
    material_sheet: bool,
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
        let preferences = ui::Preferences::load();
        let mode = if preferences.dark.unwrap_or(false) {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        };
        ui::apply_theme(mode, window, cx);
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
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(3))
                    .await;
                if this.update(cx, |this, cx| this.poll_documents(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
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
            preferences,
            navigation_sheet: false,
            material_sheet: false,
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
                            if self.knowledge.overview {
                                self.knowledge_action(KnowledgeAction::List, cx);
                            }
                            if self.knowledge.active.is_some() {
                                self.knowledge_action(KnowledgeAction::Load, cx);
                            }
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

fn main() {
    #[cfg(all(feature = "headless-tests", target_os = "windows"))]
    if std::env::args().any(|arg| arg == "--render-previews") {
        visual_preview::run();
        return;
    }
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            cx.spawn(async move |cx| {
                cx.open_window(
                    WindowOptions {
                        titlebar: Some(TitlebarOptions {
                            title: Some("Context-OS".into()),
                            appears_transparent: false,
                            traffic_light_position: None,
                        }),
                        ..Default::default()
                    },
                    |window, cx| {
                        let view = cx.new(|cx| ChatApp::new(window, cx));
                        let surface = cx.new(|cx| ui::Surface::new(view, cx));
                        cx.new(|cx| Root::new(surface, window, cx))
                    },
                )
                .expect("open desktop window");
            })
            .detach();
        });
}
