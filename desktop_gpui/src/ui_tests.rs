//! Real application views in GPUI's test platform. No OS pointer or native window.
use super::ChatApp;
use desktop_gpui::services::Phase;
use gpui_kit::component::{Root, Theme, ThemeMode};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, App, AppContext, Entity, TestAppContext, Window, px, size};
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use web_sdk::TurnStatus;

mod fixture;
mod knowledge_fixture;
mod knowledge_tests;
use fixture::Fixture;

struct Harness {
    view: Entity<ChatApp>,
    window: AnyWindowHandle,
    fixture: Fixture,
}
impl Harness {
    fn new(cx: &mut TestAppContext, width: f32, height: f32) -> Self {
        // Integration tests deliberately retain Tokio/HTTP I/O rather than a simulated Host.
        cx.executor().allow_parking();
        let fixture = Fixture::start();
        cx.update(gpui_kit::init);
        let mut view = None;
        let window = cx
            .open_window(size(px(width), px(height)), |window, cx| {
                let app = cx.new(|cx| ChatApp::new(window, cx));
                view = Some(app.clone());
                Root::new(app, window, cx)
            })
            .into();
        let harness = Self {
            view: view.unwrap(),
            window,
            fixture,
        };
        harness.wait(cx, |v| !v.services.busy());
        harness
    }
    fn frame<R>(&self, cx: &mut TestAppContext, f: impl FnOnce(&mut Window, &mut App) -> R) -> R {
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            f(window, cx)
        })
        .unwrap()
    }
    fn click(&self, cx: &mut TestAppContext, id: &'static str) {
        self.frame(cx, |window, cx| window.click(id, cx));
    }
    fn input(&self, cx: &mut TestAppContext, text: &str) {
        self.frame(cx, |window, cx| {
            window.click("composer-input", cx);
            window.input(text, cx);
        });
    }
    fn wait(&self, cx: &mut TestAppContext, mut ready: impl FnMut(&ChatApp) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            cx.run_until_parked();
            if self.frame(cx, |_, cx| ready(self.view.read(cx))) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "UI timed out: {}",
                self.view.read_with(cx, |v, _| format!(
                    "phase={:?}, notice={}, error={:?}, turn={:?}, answer={}",
                    v.services.phase,
                    v.notice,
                    v.services.error,
                    v.conversation.turn.status,
                    v.conversation.turn.answer_text
                ))
            );
            // GPUI's clock is virtual; the real Host/HTTP workers need wall-clock scheduling.
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn connect(&self, cx: &mut TestAppContext) {
        self.click(cx, "connect");
        self.wait(cx, |v| v.token.is_some() && !v.sessions.is_empty());
    }
    fn draft(&self, cx: &TestAppContext) -> String {
        self.view
            .read_with(cx, |v, cx| v.input.read(cx).value().to_string())
    }
    fn close(self, cx: &mut TestAppContext) {
        // Test window removal drops the real Host before fixture teardown.
        cx.update_window(self.window, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
    }
}

#[gpui_kit::test]
fn service_panel_keeps_draft_and_history_through_reconnect(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1280., 720.);
    h.connect(cx);
    h.input(cx, "待发送的中文草稿");
    h.click(cx, "local-services");
    h.wait(cx, |v| !v.services.busy());
    h.frame(cx, |window, _| {
        assert!(window.find("refresh-services").visible());
        assert_eq!(
            window.find("open-service-logs").label(),
            Some("打开服务运行目录")
        );
    });
    h.click(cx, "refresh-services");
    h.wait(cx, |v| !v.services.busy());
    h.click(cx, "stop-services");
    h.wait(cx, |v| {
        v.token.is_none() && v.services.phase == Phase::Stopped
    });
    assert_eq!(h.draft(cx), "待发送的中文草稿");
    h.click(cx, "start-services");
    h.wait(cx, |v| {
        v.token.is_some() && v.services.phase == Phase::Ready
    });
    h.click(cx, "back-to-chat");
    assert_eq!(h.draft(cx), "待发送的中文草稿");
    h.frame(cx, |window, _| {
        assert!(window.find("ui-history").visible());
        assert!(window.try_find("workspace-history").is_none());
    });
    h.click(cx, "ui-history");
    h.wait(cx, |v| !v.loading && v.conversation.messages.len() == 2);
    assert_eq!(h.draft(cx), "待发送的中文草稿");
    assert!(h.fixture.count("GET /health") >= 4);
    h.close(cx);
}

#[gpui_kit::test]
fn busy_connection_disables_duplicate_clicks(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1280., 720.);
    h.fixture.hold_auth.store(true, Ordering::SeqCst);
    h.click(cx, "connect");
    h.wait(cx, |v| v.services.phase == Phase::Connecting);
    h.frame(cx, |window, cx| {
        // Kit 0.6 suppresses disabled button events but does not export aria-disabled.
        // Verify actual dispatch/request behavior below, not invented snapshot properties.
        window.click("connect", cx);
    });
    h.click(cx, "local-services");
    h.frame(cx, |window, cx| {
        window.click("start-services", cx);
        window.click("refresh-services", cx);
    });
    h.view
        .read_with(cx, |v, _| assert_eq!(v.services.phase, Phase::Connecting));
    h.fixture.hold_auth.store(false, Ordering::SeqCst);
    h.wait(cx, |v| v.token.is_some());
    assert_eq!(h.fixture.count("POST /api/auth/login"), 1);
    h.close(cx);
}

#[gpui_kit::test]
fn failed_health_can_retry_without_losing_input(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1280., 720.);
    h.input(cx, "连接失败也保留");
    h.fixture.healthy.store(false, Ordering::SeqCst);
    h.click(cx, "connect");
    h.wait(cx, |v| v.services.phase == Phase::Failed);
    assert_eq!(h.fixture.count("POST /api/auth/login"), 0);
    assert_eq!(h.draft(cx), "连接失败也保留");
    h.frame(cx, |window, _| {
        assert_eq!(window.find("connect").label(), Some("重试连接"))
    });
    h.fixture.healthy.store(true, Ordering::SeqCst);
    h.connect(cx);
    assert_eq!(h.draft(cx), "连接失败也保留");
    h.close(cx);
}

#[gpui_kit::test]
fn send_stream_stop_and_next_turn_use_real_controls(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1280., 720.);
    h.connect(cx);
    h.input(cx, "第一轮问题");
    h.click(cx, "send");
    h.wait(cx, |v| v.conversation.turn.answer_text == "第一段中文");
    assert!(h.draft(cx).is_empty());
    h.frame(cx, |window, _| assert!(window.find("stop").visible()));
    h.click(cx, "stop");
    h.wait(cx, |v| {
        v.cancel.is_none() && v.conversation.turn.status == TurnStatus::Cancelled
    });
    h.fixture.finish_stream.store(true, Ordering::SeqCst);
    h.input(cx, "下一轮问题");
    h.click(cx, "send");
    h.wait(cx, |v| v.conversation.turn.status == TurnStatus::Done);
    h.view.read_with(cx, |v, _| {
        assert_eq!(v.conversation.messages[1].1, "第一段中文");
        assert_eq!(v.conversation.turn.answer_text, "第一段中文，完成回答。");
    });
    assert_eq!(h.fixture.count("POST /api/v1/chat"), 2);
    h.close(cx);
}

#[gpui_kit::test]
fn keyboard_editing_and_return_focus_preserve_draft(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1280., 720.);
    h.input(cx, "中文abc");
    h.frame(cx, |window, cx| {
        window.press("backspace", cx);
        assert_eq!(window.find("composer-input").focused(), Some(true));
    });
    assert_eq!(h.draft(cx), "中文ab");
    h.click(cx, "local-services");
    h.wait(cx, |v| !v.services.busy());
    h.click(cx, "back-to-chat");
    h.frame(cx, |window, cx| {
        assert_eq!(window.find("composer-input").focused(), Some(true));
        window.input("继续", cx);
    });
    assert_eq!(h.draft(cx), "中文ab继续");
    h.close(cx);
}

#[gpui_kit::test]
fn loading_history_blocks_send_and_new_chat_recovers_controls(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1280., 720.);
    h.connect(cx);
    h.input(cx, "不能在加载期间发出");
    // A controlled in-flight history state exercises the real disabled button.
    h.view.update(cx, |v, cx| {
        v.loading = true;
        cx.notify();
    });
    h.frame(cx, |window, cx| {
        window.click("send", cx);
    });
    assert_eq!(h.fixture.count("POST /api/v1/chat"), 0);
    assert_eq!(h.draft(cx), "不能在加载期间发出");
    h.click(cx, "new-chat");
    h.view.read_with(cx, |v, _| {
        assert!(!v.loading);
        assert!(v.conversation.messages.is_empty());
        assert!(v.conversation.session_id.is_none());
    });
    h.close(cx);
}

#[gpui_kit::test]
fn chat_layout_keeps_controls_inside_short_and_narrow_windows(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1280., 720.);
    h.connect(cx);
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        for (width, height) in [(1440., 900.), (1280., 720.), (768., 600.), (640., 480.)] {
            cx.simulate_window_resize(h.window, size(px(width), px(height)));
            h.frame(cx, |window, cx| Theme::change(mode, Some(window), cx));
            for long in [false, true] {
                h.view.update(cx, |v, cx| {
                    v.conversation.messages = if long {
                        vec![("assistant".into(), format!("# 长回答\n\n{}\n\n```rust\n{}\n```\n\n| 名称 | 内容 |\n| --- | --- |\n| 项目 | {} |", "用于验证滚动和布局的中文正文。\n\n".repeat(60), "long_code_".repeat(80), "表格内容".repeat(80)))]
                    } else { vec![] };
                    cx.notify();
                });
                if long {
                    h.wait(cx, |v| v.scroll.max_offset().y > px(0.));
                }
                h.frame(cx, |window, cx| {
                    let send = window.find("send");
                    let input = window.find("composer-input");
                    let messages = window.find("messages");
                    assert!(
                        send.visible() && input.visible(),
                        "controls clipped at {width}x{height}"
                    );
                    assert!(send.bounds().bottom() <= px(height), "send below viewport");
                    assert!(send.bounds().right() <= px(width), "send beyond right edge");
                    assert!(input.bounds().right() <= px(width));
                    assert!(input.bounds().size.width > px(180.));
                    assert!(messages.bounds().bottom() <= input.bounds().top());
                    if long {
                        assert!(h.view.read(cx).scroll.max_offset().y > px(0.));
                        window.scroll(
                            "messages",
                            gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-500.))),
                            cx,
                        );
                        assert!(h.view.read(cx).scroll.offset().y < px(0.));
                        assert!(window.find("send").visible());
                    }
                });
            }
        }
    }
    h.close(cx);
}

#[gpui_kit::test]
fn window_close_waits_for_connection_and_preserves_external_api(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1280., 720.);
    h.fixture.hold_auth.store(true, Ordering::SeqCst);
    h.click(cx, "connect");
    h.wait(cx, |v| v.services.phase == Phase::Connecting);
    let mut window = gpui_kit::VisualTestContext::from_window(h.window, cx);
    assert!(
        !window.simulate_close(),
        "closing must wait for the in-flight connection"
    );
    h.view.read_with(cx, |v, _| {
        assert!(v.exiting && !v.exit_ready);
        assert_eq!(v.services.phase, Phase::Stopping);
    });
    h.fixture.hold_auth.store(false, Ordering::SeqCst);
    h.wait(cx, |v| v.exit_ready);
    assert!(window.simulate_close());
    h.view.read_with(cx, |v, _| {
        assert!(
            v.token.is_none(),
            "late login must not reconnect during exit"
        );
        let status = v.services.snapshot.as_ref().unwrap();
        assert!(status.product.api_ok && status.attached && !status.owned);
    });
    h.close(cx);
}

#[gpui_kit::test]
fn service_panel_actions_remain_reachable_in_short_windows(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1280., 720.);
    h.connect(cx);
    h.input(cx, "短屏草稿");
    h.click(cx, "local-services");
    h.wait(cx, |v| !v.services.busy());
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        for (width, height) in [(1280., 720.), (768., 600.), (640., 480.)] {
            cx.simulate_window_resize(h.window, size(px(width), px(height)));
            h.frame(cx, |window, cx| {
                Theme::change(mode, Some(window), cx);
                window.render_frame(cx);
                let body = window.find("service-content");
                let back = window.find("back-to-chat");
                assert!(body.bounds().right() <= px(width));
                assert!(body.bounds().bottom() <= px(height));
                assert!(back.visible());
                window.scroll(
                    "service-content",
                    gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-4000.))),
                    cx,
                );
                for id in ["refresh-services", "stop-services"] {
                    let button = window.find(id);
                    assert!(button.visible(), "{id} unreachable at {width}x{height}");
                    assert!(button.bounds().right() <= px(width));
                    assert!(button.bounds().bottom() <= px(height));
                }
            });
        }
    }
    h.click(cx, "stop-services");
    h.wait(cx, |v| v.services.phase == Phase::Stopped);
    h.click(cx, "start-services");
    h.wait(cx, |v| v.services.phase == Phase::Ready);
    h.click(cx, "back-to-chat");
    assert_eq!(h.draft(cx), "短屏草稿");
    h.close(cx);
}
