//! Acceptance-only DirectX readback of hidden windows. Never captures the desktop.
use crate::*;
use std::{path::PathBuf, time::Duration};

#[path = "ui_tests/fixture.rs"]
mod fixture;
#[path = "ui_tests/knowledge_fixture.rs"]
mod knowledge_fixture;

fn populate(app: &mut ChatApp, scene: &str, window: &mut Window, cx: &mut Context<ChatApp>) {
    use serde_json::json;
    app.preferences = Default::default();
    app.token = Some("synthetic-preview-session".into());
    app.notice.clear();
    app.services.phase = Phase::Ready;
    let workspace: contracts::workspaces::Workspace = serde_json::from_value(json!({
        "id":"workspace-ui", "owner_user_id":"ui-user", "owner_id":"ui-user", "name":"产品研究",
        "title":"产品研究", "description":"", "created_at":"2026-09-10", "updated_at":"2026-09-10", "document_count":2
    })).unwrap();
    app.knowledge.workspaces = vec![workspace.clone()];
    let mut second = workspace.clone();
    second.id = "workspace-two".into();
    second.name = "阅读与思考".into();
    app.knowledge.workspaces.push(second);
    for (id, title) in [
        ("first", "如何整理产品调研资料？"),
        ("second", "本周值得关注的三个方向"),
        ("third", "把想法整理成行动清单"),
    ] {
        app.sessions.push(serde_json::from_value(json!({"id":id,"title":title,"workspace_id":null,
            "owner_user_id":"ui-user","scope_kind":"personal","model_role":"quick_chat","agent_type":"quick_chat",
            "created_at":"2026-09-10","updated_at":"2026-09-10"})).unwrap());
    }
    if scene == "chat" || scene.starts_with("workspace-") {
        app.conversation.session_id = Some("first".into());
        app.conversation.messages = vec![
            ("user".into(), "如何把零散的调研资料整理成可以持续使用的知识？".into()),
            ("assistant".into(), "可以先围绕**一个明确的问题**建立工作区，让资料与结论各有位置。\n\n### 从三个步骤开始\n\n1. **收集资料**：把文档、表格和演示文稿放在同一个工作区。\n2. **带着问题阅读**：先问一个具体问题，再沿着引用核对原文。\n3. **记录自己的判断**：用笔记保存结论和下一步行动。\n\n资料负责保留依据，笔记负责承载你的思考。后续继续提问时，也可以只选择相关的几份资料。".into()),
        ];
        app.conversation.turn.status = TurnStatus::Done;
    }
    app.knowledge.overview = scene == "workspaces";
    app.show_services = scene == "services";
    if scene.starts_with("workspace-") {
        app.knowledge.active = Some(workspace);
        app.knowledge.documents = [("research.docx", "completed"), ("访谈记录与需求整理.xlsx", "failed")].iter().enumerate().map(|(i, (name, status))| {
            serde_json::from_value(json!({"id":format!("preview-doc-{i}"),"workspace_id":"workspace-ui","owner_user_id":"ui-user",
                "owner_id":"ui-user","file_name":name,"mime_type":"application/octet-stream","file_size":4096,
                "status":status,"chunk_count":1,"created_at":"2026-09-10","updated_at":"2026-09-10"})).unwrap()
        }).collect();
        app.knowledge.panel = Some(if scene == "workspace-notes" {
            Panel::Notes
        } else {
            Panel::Documents
        });
        if scene == "workspace-notes" {
            app.knowledge
                .note_title
                .update(cx, |v, cx| v.set_value("调研结论与下一步", window, cx));
            app.knowledge.note_content.update(cx, |v, cx| v.set_value("先验证用户的核心问题，再补充解决方案。\n\n下一步：\n• 整理访谈中的共同需求\n• 核对引用与原始资料\n• 形成可执行的行动清单", window, cx));
        }
    }
    cx.notify();
}

async fn capture(cx: &mut AsyncApp, directory: &std::path::Path) -> Result<Vec<String>, String> {
    let mut files = vec![];
    let mut app = None;
    let handle = cx
        .open_window(
            WindowOptions {
                show: false,
                focus: false,
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| ChatApp::new(window, cx));
                app = Some(view.clone());
                let surface = cx.new(|cx| ui::Surface::new(view, cx));
                cx.new(|cx| Root::new(surface, window, cx))
            },
        )
        .map_err(|e| e.to_string())?;
    let app = app.unwrap();
    for (mode, theme) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
        for (width, height) in [(1280., 800.), (390., 720.)] {
            for scene in [
                "empty",
                "chat",
                "workspaces",
                "workspace-documents",
                "workspace-notes",
                "services",
            ] {
                // Windows defers the initial placement of hidden windows. An explicit
                // hidden resize allocates the render target without showing the window.
                cx.update_window(handle.into(), |_, window, cx| {
                    window.close_sheet(cx);
                    window.resize(size(px(width), px(height)));
                    ui::apply_theme(mode, window, cx);
                    app.update(cx, |app, cx| {
                        app.navigation_sheet = false;
                        app.material_sheet = false;
                        app.sessions.clear();
                        app.conversation.reset(None);
                        app.knowledge.active = None;
                        app.knowledge.panel = None;
                        app.knowledge.documents.clear();
                        app.set_note(None, window, cx);
                        populate(app, scene, window, cx);
                    });
                })
                .map_err(|e| e.to_string())?;
                cx.background_executor()
                    .timer(Duration::from_millis(180))
                    .await;
                for _ in 0..6 {
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.refresh();
                        let _ = window.draw(cx);
                    })
                    .map_err(|e| e.to_string())?;
                    cx.background_executor()
                        .timer(Duration::from_millis(60))
                        .await;
                }
                let path = directory.join(format!("{scene}-{width:.0}-{theme}.png"));
                cx.update_window(handle.into(), |_, window, _| {
                    let image = window.render_to_image().map_err(|e| e.to_string())?;
                    if image.width() < width as u32 || image.height() < height as u32 {
                        return Err(format!(
                            "Unexpected render target {}x{} for {width}x{height}",
                            image.width(),
                            image.height()
                        ));
                    }
                    image.save(&path).map_err(|e| e.to_string())
                })
                .map_err(|e| e.to_string())??;
                files.push(path.to_string_lossy().into_owned());
            }
        }
    }
    // The application owns the one hidden window until receipt writing finishes.
    // Closing the last native window mid-run would terminate the Windows event loop.
    Ok(files)
}
pub fn run() {
    assert_eq!(std::env::var("GPUI_ACCEPTANCE_UI").as_deref(), Ok("1"));
    let directory = PathBuf::from(
        std::env::var_os("CONTEXT_OS_CLIENT_HOME").expect("isolated preview directory"),
    );
    assert!(
        directory
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("gpui-ui-")
    );
    let fixture = fixture::Fixture::start();
    gpui_kit::application().with_assets(gpui_kit::assets::Assets).run(move |cx| {
        gpui_kit::init(cx);
        cx.spawn(async move |cx| {
            let result = capture(cx, &directory).await;
            let receipt = match result {
                Ok(files) => serde_json::json!({"ok":true,"renderer":"Windows DirectX, hidden windows, synthetic fixture data","files":files}),
                Err(error) => serde_json::json!({"ok":false,"error":error}),
            };
            std::fs::write(directory.join("visual-result.json"), serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();
            drop(fixture);
            cx.update(|cx| cx.quit());
        }).detach();
    });
}
