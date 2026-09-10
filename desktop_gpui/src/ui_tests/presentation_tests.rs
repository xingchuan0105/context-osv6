use super::*;
use gpui_kit::component::WindowExt;
use std::io::Write;

fn record_layout(
    window: &Window,
    scene: &str,
    width: f32,
    height: f32,
    mode: ThemeMode,
    ids: &[&'static str],
) {
    let elements = ids
        .iter()
        .map(|id| {
            let element = window.find(*id);
            let bounds = element.bounds();
            serde_json::json!({"id": id, "label": element.label(), "visible": element.visible(),
            "x": f32::from(bounds.left()), "y": f32::from(bounds.top()),
            "width": f32::from(bounds.size.width), "height": f32::from(bounds.size.height)})
        })
        .collect::<Vec<_>>();
    let path = std::path::PathBuf::from(std::env::var_os("CONTEXT_OS_CLIENT_HOME").unwrap())
        .join("layout-matrix.jsonl");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    writeln!(file, "{}", serde_json::json!({"scene": scene, "width": width, "height": height,
        "theme": format!("{mode:?}"), "evidence": "GPUI TestPlatform geometry, not GPU pixels", "elements": elements})).unwrap();
}

#[gpui_kit::test]
fn navigation_and_theme_preferences_keep_drafts_and_scope(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1440., 900.);
    h.connect(cx);
    h.input(cx, "外观切换保留草稿");
    h.frame(cx, |window, _| {
        assert_eq!(window.find("sidebar").bounds().size.width, px(248.))
    });
    h.click(cx, "toggle-navigation");
    h.frame(cx, |window, _| {
        assert_eq!(window.find("sidebar").bounds().size.width, px(60.));
        assert_eq!(window.find("personal-nav").label(), Some("个人聊天"));
        assert_eq!(window.find("workspaces-nav").label(), Some("工作区"));
        assert!(
            window.find("workspaces-nav").bounds().top()
                > window.find("personal-nav").bounds().bottom()
        );
    });
    h.click(cx, "toggle-theme");
    h.view.read_with(cx, |v, _| {
        assert_eq!(v.preferences.dark, Some(true));
        assert_eq!(v.preferences.collapsed, Some(true));
        assert!(v.preferences.error.is_none());
    });
    let saved = crate::ui::Preferences::load();
    assert_eq!(saved.dark, Some(true));
    assert_eq!(saved.collapsed, Some(true));
    assert_eq!(h.draft(cx), "外观切换保留草稿");
    h.click(cx, "workspaces-nav");
    h.wait(cx, |v| {
        v.knowledge.overview && v.knowledge.workspaces.len() == 2
    });
    h.click(cx, "personal-nav");
    assert_eq!(h.draft(cx), "外观切换保留草稿");
    // Other cases start from the product's default appearance, not this case's preference.
    h.view.update(cx, |v, _| {
        v.preferences = Default::default();
        v.preferences.save();
    });
    h.close(cx);
}

#[gpui_kit::test]
fn mobile_navigation_traps_focus_and_blocks_background_send(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 390., 720.);
    h.connect(cx);
    h.input(cx, "不能被抽屉穿透发送");
    h.frame(cx, |window, _| {
        assert!(window.try_find("sidebar").is_none())
    });
    h.click(cx, "open-navigation");
    h.frame(cx, |window, cx| {
        assert!(window.has_active_sheet(cx));
        assert!(window.find("workspaces-nav").visible());
        for _ in 0..16 {
            window.press("tab", cx);
            assert_ne!(window.find("composer-input").focused(), Some(true));
        }
        // This dispatches a real pointer event at Send, which must hit the modal backdrop.
        window.click("send", cx);
    });
    assert_eq!(h.fixture.count("POST /api/v1/chat"), 0);
    assert_eq!(h.draft(cx), "不能被抽屉穿透发送");
    h.click(cx, "open-navigation");
    h.frame(cx, |window, cx| {
        window.press("escape", cx);
        assert!(!window.has_active_sheet(cx));
    });
    h.click(cx, "open-navigation");
    h.click(cx, "workspaces-nav");
    h.wait(cx, |v| {
        v.knowledge.overview && v.knowledge.workspaces.len() == 2
    });
    h.frame(cx, |window, cx| assert!(!window.has_active_sheet(cx)));
    h.click(cx, "open-navigation");
    h.click(cx, "personal-nav");
    assert_eq!(h.draft(cx), "不能被抽屉穿透发送");
    h.close(cx);
}

#[gpui_kit::test]
fn empty_chat_is_grouped_and_composer_has_one_toolbar(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1440., 900.);
    h.connect(cx);
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        for (width, height) in [(1440., 900.), (1280., 720.), (768., 600.), (390., 720.)] {
            cx.simulate_window_resize(h.window, size(px(width), px(height)));
            h.frame(cx, |window, cx| {
                crate::ui::apply_theme(mode, window, cx);
                window.render_frame(cx);
                let welcome = window.find("chat-welcome").bounds();
                let composer = window.find("composer").bounds();
                let toolbar = window.find("composer-toolbar").bounds();
                let send = window.find("send").bounds();
                assert!(composer.top() - welcome.bottom() >= px(0.));
                assert!(composer.top() - welcome.bottom() <= px(24.));
                assert!(composer.size.width <= px(760.));
                assert!(toolbar.left() >= composer.left() && toolbar.right() <= composer.right());
                assert!(send.top() >= toolbar.top() && send.bottom() <= toolbar.bottom());
                assert_eq!(window.find("context-header").bounds().size.height, px(52.));
                assert!(window.try_find("workspace-documents").is_none());
                record_layout(
                    window,
                    "chat-empty",
                    width,
                    height,
                    mode,
                    &[
                        "context-header",
                        "chat-welcome",
                        "composer",
                        "composer-toolbar",
                        "send",
                    ],
                );
            });
        }
    }
    h.close(cx);
}

#[gpui_kit::test]
fn overview_and_service_pages_fit_desktop_and_phone_in_both_themes(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1280., 800.);
    h.connect(cx);
    h.click(cx, "workspaces-nav");
    h.wait(cx, |v| v.knowledge.workspaces.len() == 2);
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        for (width, height) in [(1440., 900.), (1280., 720.), (768., 600.), (390., 720.)] {
            cx.simulate_window_resize(h.window, size(px(width), px(height)));
            h.frame(cx, |window, cx| {
                crate::ui::apply_theme(mode, window, cx);
                window.render_frame(cx);
                for id in [
                    "create-workspace",
                    "workspace-name",
                    "workspace-workspace-ui",
                    "workspace-workspace-two",
                ] {
                    let bounds = window.find(id).bounds();
                    assert!(
                        bounds.left() >= px(0.) && bounds.right() <= px(width),
                        "{id} beyond {width}"
                    );
                }
                assert!(window.find("workspace-overview").bounds().size.width <= px(1120.));
                record_layout(
                    window,
                    "workspaces",
                    width,
                    height,
                    mode,
                    &[
                        "context-header",
                        "workspace-overview",
                        "create-workspace",
                        "workspace-name",
                        "workspace-workspace-ui",
                    ],
                );
            });
        }
    }
    h.click(cx, "local-services");
    h.wait(cx, |v| !v.services.busy());
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        for (width, height) in [(1440., 900.), (1280., 720.), (768., 600.), (390., 720.)] {
            cx.simulate_window_resize(h.window, size(px(width), px(height)));
            h.frame(cx, |window, cx| {
                crate::ui::apply_theme(mode, window, cx);
                window.render_frame(cx);
                window.scroll(
                    "service-content",
                    gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-4000.))),
                    cx,
                );
                for id in ["back-to-chat", "refresh-services", "stop-services"] {
                    let button = window.find(id);
                    assert!(button.visible(), "{id} unreachable at {width}");
                    assert!(button.bounds().right() <= px(width));
                }
                record_layout(
                    window,
                    "services",
                    width,
                    height,
                    mode,
                    &[
                        "context-header",
                        "service-content",
                        "back-to-chat",
                        "refresh-services",
                        "stop-services",
                    ],
                );
            });
        }
    }
    h.close(cx);
}

#[gpui_kit::test]
fn workspace_drawer_keeps_note_draft_across_tabs_resize_and_escape(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1280., 800.);
    super::knowledge_tests::enter(&h, cx);
    h.click(cx, "workspace-notes");
    super::knowledge_tests::edit(&h, cx, "note-content", "草稿随资料面板保留");
    h.click(cx, "panel-documents");
    h.click(cx, "panel-notes");
    for (width, height) in [(768., 720.), (390., 720.)] {
        cx.simulate_window_resize(h.window, size(px(width), px(height)));
        // Kit's slide animation also consumes native frame timestamps. Wait for the
        // completed position, while advancing the test dispatcher and real frames.
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let settled = h.frame(cx, |window, _| {
                window
                    .try_find("knowledge-panel")
                    .is_some_and(|panel| panel.bounds().right() <= px(width))
            });
            if settled {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "drawer did not settle inside {width}"
            );
            cx.executor().advance_clock(Duration::from_millis(16));
            std::thread::sleep(Duration::from_millis(20));
        }
        h.frame(cx, |window, cx| {
            window.render_frame(cx);
            assert!(window.has_active_sheet(cx));
            let panel = window.find("knowledge-panel").bounds();
            assert!(
                panel.right() <= px(width),
                "panel {:?} beyond {width}",
                panel
            );
            window.scroll(
                "knowledge-panel-scroll",
                gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-2000.))),
                cx,
            );
            assert!(window.find("save-note").visible());
            window.press("escape", cx);
            assert!(!window.has_active_sheet(cx));
        });
        h.view.read_with(cx, |v, cx| {
            assert_eq!(
                v.knowledge.note_content.read(cx).value().as_ref(),
                "草稿随资料面板保留"
            )
        });
        h.click(cx, "workspace-notes");
    }
    cx.simulate_window_resize(h.window, size(px(1440.), px(900.)));
    h.frame(cx, |window, cx| {
        window.render_frame(cx);
        assert!(!window.has_active_sheet(cx));
        assert_eq!(window.find("knowledge-panel").bounds().size.width, px(336.));
        assert!(window.find("send").visible());
    });
    h.close(cx);
}
