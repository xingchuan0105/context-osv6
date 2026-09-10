use super::*;
use crate::knowledge_view::Panel;
use serde_json::json;

pub(super) fn enter(h: &Harness, cx: &mut TestAppContext) {
    h.connect(cx);
    h.click(cx, "workspaces-nav");
    h.wait(cx, |v| v.knowledge.workspaces.len() == 2);
    h.click(cx, "workspace-workspace-ui");
    h.wait(cx, |v| {
        v.knowledge.documents.len() == 2 && v.sessions.len() == 1
    });
}
pub(super) fn edit(h: &Harness, cx: &mut TestAppContext, id: &'static str, value: &str) {
    h.frame(cx, |window, cx| {
        window.click(id, cx);
        window.input(value, cx);
    });
}

#[gpui_kit::test]
fn saved_note_update_uses_the_real_put_contract(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1280., 800.);
    enter(&h, cx);
    h.click(cx, "workspace-notes");
    edit(&h, cx, "note-title", "保存后修改");
    edit(&h, cx, "note-content", "第一版");
    h.click(cx, "save-note");
    h.wait(cx, |v| v.knowledge.note_id.is_some());
    edit(&h, cx, "note-content", "补充内容");
    let updated = h.view.read_with(cx, |v, cx| {
        v.knowledge.note_content.read(cx).value().to_string()
    });
    assert!(updated.contains("补充内容"));
    h.click(cx, "save-note");
    h.wait(cx, |v| {
        v.knowledge.notes.iter().any(|n| n.content == updated)
    });
    assert_eq!(
        h.fixture
            .count("POST /api/v1/workspaces/workspace-ui/notes"),
        1
    );
    assert_eq!(
        h.fixture
            .count("PUT /api/v1/workspaces/workspace-ui/notes/created-note"),
        1
    );
    assert_eq!(
        h.fixture
            .count("PATCH /api/v1/workspaces/workspace-ui/notes/created-note"),
        0
    );
    h.close(cx);
}

#[gpui_kit::test]
fn workspace_create_and_navigation_keep_personal_draft(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1280., 720.);
    h.connect(cx);
    h.input(cx, "个人草稿");
    h.click(cx, "workspaces-nav");
    h.wait(cx, |v| v.knowledge.workspaces.len() == 2);
    edit(&h, cx, "workspace-name", "新建资料空间");
    h.click(cx, "create-workspace");
    h.wait(cx, |v| {
        v.knowledge
            .active
            .as_ref()
            .is_some_and(|w| w.id == "created-workspace")
            && !v.knowledge.busy(|_| true)
    });
    assert_eq!(h.draft(cx), "");
    h.input(cx, "工作区草稿");
    h.click(cx, "personal-nav");
    h.wait(cx, |v| v.sessions.iter().any(|s| s.id == "ui-history"));
    assert_eq!(h.draft(cx), "个人草稿");
    h.click(cx, "workspaces-nav");
    h.wait(cx, |v| v.knowledge.workspaces.len() == 3);
    h.click(cx, "workspace-created-workspace");
    assert_eq!(h.draft(cx), "工作区草稿");
    assert_eq!(h.fixture.count("POST /api/v1/workspaces"), 1);
    h.close(cx);
}

#[gpui_kit::test]
fn workspace_rag_scope_and_citation_do_not_leak_into_personal_chat(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1440., 900.);
    enter(&h, cx);
    h.click(cx, "workspace-documents");
    h.click(cx, "select-doc-failed-document");
    h.view
        .read_with(cx, |v, _| assert!(v.knowledge.selected.is_empty()));
    h.click(cx, "select-doc-ui-document");
    h.click(cx, "close-knowledge-panel");
    h.fixture.finish_stream.store(true, Ordering::SeqCst);
    h.input(cx, "根据工作区资料回答");
    h.click(cx, "send");
    h.wait(cx, |v| v.conversation.turn.status == TurnStatus::Done);
    {
        let requests = h.fixture.requests.lock().unwrap();
        let request = &requests
            .iter()
            .find(|(r, _)| r == "POST /api/v1/chat")
            .unwrap()
            .1;
        assert_eq!(request["workspace_id"], "workspace-ui");
        assert_eq!(request["capabilities"], json!(["rag"]));
        assert_eq!(request["doc_scope"], json!(["ui-document"]));
    }
    h.click(cx, "citation-current-0");
    h.wait(cx, |v| {
        v.knowledge
            .preview
            .as_ref()
            .is_some_and(|(_, text)| text.contains("原文核对内容"))
    });
    h.click(cx, "close-knowledge-panel");
    h.click(cx, "personal-nav");
    h.input(cx, "快速提问");
    h.click(cx, "send");
    h.wait(cx, |v| v.conversation.turn.status == TurnStatus::Done);
    let request = h
        .fixture
        .requests
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find(|(r, _)| r == "POST /api/v1/chat")
        .unwrap()
        .1
        .clone();
    assert_eq!(request["capabilities"], json!([]));
    assert!(request.get("workspace_id").is_none());
    assert!(request.get("doc_scope").is_none());
    h.close(cx);
}

#[gpui_kit::test]
fn note_save_failure_collapse_and_navigation_preserve_draft(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1280., 800.);
    enter(&h, cx);
    h.click(cx, "workspace-notes");
    edit(&h, cx, "note-title", "会议记录");
    edit(&h, cx, "note-content", "未保存的笔记正文");
    h.click(cx, "close-knowledge-panel");
    h.click(cx, "workspace-notes");
    h.view.read_with(cx, |v, cx| {
        assert_eq!(
            v.knowledge.note_content.read(cx).value().as_ref(),
            "未保存的笔记正文"
        )
    });
    h.fixture
        .knowledge
        .lock()
        .unwrap()
        .fail_notes
        .store(true, Ordering::SeqCst);
    h.click(cx, "save-note");
    h.wait(cx, |v| v.knowledge.error.is_some());
    h.click(cx, "workspaces-nav");
    h.view
        .read_with(cx, |v, _| assert!(v.knowledge.confirm.is_some()));
    h.click(cx, "cancel-knowledge-confirm");
    h.view.read_with(cx, |v, cx| assert!(v.knowledge.dirty(cx)));
    h.fixture
        .knowledge
        .lock()
        .unwrap()
        .fail_notes
        .store(false, Ordering::SeqCst);
    h.click(cx, "save-note");
    h.wait(cx, |v| v.knowledge.note_id.is_some());
    h.view
        .read_with(cx, |v, cx| assert!(!v.knowledge.dirty(cx)));
    h.click(cx, "delete-note");
    h.click(cx, "cancel-knowledge-confirm");
    h.view
        .read_with(cx, |v, _| assert_eq!(v.knowledge.notes.len(), 1));
    h.click(cx, "delete-note");
    h.click(cx, "accept-knowledge-confirm");
    h.wait(cx, |v| {
        v.knowledge.notes.is_empty() && v.knowledge.note_id.is_none()
    });
    h.close(cx);
}

#[gpui_kit::test]
fn file_picker_cancel_and_real_upload_bytes_poll_to_ready(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1280., 800.);
    enter(&h, cx);
    assert!(h.view.read_with(cx, |v, _| v.knowledge.panel.is_none()));
    h.click(cx, "add-workspace-document");
    assert!(cx.did_prompt_for_paths());
    cx.simulate_path_prompt_response(|_| None);
    cx.run_until_parked();
    assert!(h.view.read_with(cx, |v, _| v.knowledge.panel.is_none()));
    let root = std::path::PathBuf::from(std::env::var_os("CONTEXT_OS_DESKTOP_DATA_DIR").unwrap());
    let file = root.join("办公资料-长文件名用于验证按钮和面板不会被撑开.txt");
    std::fs::write(&file, "synthetic office material").unwrap();
    h.click(cx, "add-workspace-document");
    cx.simulate_path_prompt_response(move |options| {
        assert!(options.files && !options.directories && !options.multiple);
        Some(vec![file])
    });
    h.wait(cx, |v| {
        v.knowledge
            .documents
            .iter()
            .any(|d| d.id == "uploaded-document")
    });
    assert_eq!(h.fixture.count("PUT /uploads/uploaded-document"), 1);
    h.click(cx, "close-knowledge-panel");
    // GPUI's periodic timer uses its test clock; advancing wall time alone cannot fire it.
    cx.executor().advance_clock(Duration::from_secs(3));
    h.wait(cx, |v| {
        v.knowledge
            .documents
            .iter()
            .any(|d| d.id == "uploaded-document" && d.status == "completed")
    });
    assert!(h.view.read_with(cx, |v, _| v.knowledge.panel.is_none()));
    h.close(cx);
}

fn pick_upload(h: &Harness, cx: &mut TestAppContext) {
    let file = std::path::PathBuf::from(std::env::var_os("CONTEXT_OS_DESKTOP_DATA_DIR").unwrap())
        .join("submit-recovery.txt");
    std::fs::write(&file, "synthetic submission recovery").unwrap();
    h.click(cx, "add-workspace-document");
    cx.simulate_path_prompt_response(move |_| Some(vec![file]));
}

#[gpui_kit::test]
fn failed_submit_reuses_uploaded_document_after_workspace_switch(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1280., 800.);
    enter(&h, cx);
    h.fixture
        .knowledge
        .lock()
        .unwrap()
        .fail_complete
        .store(true, Ordering::SeqCst);
    pick_upload(&h, cx);
    h.wait(cx, |v| {
        v.knowledge
            .uploads
            .values()
            .any(|u| u.error.is_some() && u.document_id.is_some())
    });
    h.click(cx, "personal-nav");
    h.click(cx, "workspaces-nav");
    h.wait(cx, |v| v.knowledge.workspaces.len() == 2);
    h.click(cx, "workspace-workspace-ui");
    h.click(cx, "workspace-documents");
    let id = h
        .view
        .read_with(cx, |v, _| *v.knowledge.uploads.keys().next().unwrap());
    h.fixture
        .knowledge
        .lock()
        .unwrap()
        .fail_complete
        .store(false, Ordering::SeqCst);
    h.frame(cx, |window, cx| {
        window.click(
            gpui_kit::SharedString::from(format!("retry-upload-{id}")),
            cx,
        )
    });
    h.wait(cx, |v| {
        v.knowledge
            .documents
            .iter()
            .any(|d| d.id == "uploaded-document" && d.status != "pending")
            && !v
                .knowledge
                .busy(|a| matches!(a, desktop_gpui::workspace::Action::CompleteUpload(_)))
    });
    h.view.read_with(cx, |v, _| {
        assert!(
            v.knowledge
                .uploads
                .values()
                .all(|u| !u.running && u.error.is_none())
        )
    });
    assert_eq!(
        h.fixture
            .count("POST /api/v1/workspaces/workspace-ui/documents"),
        1
    );
    assert_eq!(h.fixture.count("PUT /uploads/uploaded-document"), 1);
    assert_eq!(
        h.fixture
            .count("POST /api/v1/documents/uploaded-document/complete-upload"),
        2
    );
    assert_eq!(
        h.fixture
            .count("DELETE /api/v1/documents/uploaded-document"),
        0
    );
    h.close(cx);
}

#[gpui_kit::test]
fn lost_submit_response_reconciles_without_duplicate_upload(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1280., 800.);
    enter(&h, cx);
    h.fixture
        .knowledge
        .lock()
        .unwrap()
        .lose_complete_response
        .store(true, Ordering::SeqCst);
    pick_upload(&h, cx);
    h.wait(cx, |v| {
        v.knowledge
            .documents
            .iter()
            .any(|d| d.id == "uploaded-document" && d.status == "completed")
    });
    h.view.read_with(cx, |v, _| {
        assert!(v.knowledge.error.is_none());
        assert!(
            v.knowledge
                .uploads
                .values()
                .all(|u| !u.running && u.error.is_none())
        );
    });
    assert_eq!(
        h.fixture
            .count("POST /api/v1/workspaces/workspace-ui/documents"),
        1
    );
    assert_eq!(h.fixture.count("PUT /uploads/uploaded-document"), 1);
    assert_eq!(
        h.fixture
            .count("POST /api/v1/documents/uploaded-document/complete-upload"),
        1
    );
    assert_eq!(
        h.fixture
            .count("DELETE /api/v1/documents/uploaded-document"),
        0
    );
    h.close(cx);
}

#[gpui_kit::test]
fn pending_document_can_submit_without_in_memory_upload_task(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1280., 800.);
    enter(&h, cx);
    h.fixture
        .knowledge
        .lock()
        .unwrap()
        .fail_complete
        .store(true, Ordering::SeqCst);
    pick_upload(&h, cx);
    h.wait(cx, |v| {
        v.knowledge
            .documents
            .iter()
            .any(|d| d.id == "uploaded-document")
            && !v.knowledge.busy(|_| true)
    });
    // The server record is the recovery source when the prior host task is unavailable.
    h.view.update(cx, |v, cx| {
        v.knowledge.uploads.clear();
        cx.notify();
    });
    h.fixture
        .knowledge
        .lock()
        .unwrap()
        .fail_complete
        .store(false, Ordering::SeqCst);
    h.frame(cx, |window, cx| {
        window.scroll(
            "knowledge-panel-scroll",
            gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-2000.))),
            cx,
        );
        let button = window.find("complete-doc-uploaded-document");
        let scroll = window.find("knowledge-panel-scroll");
        assert!(button.visible() && button.bounds().bottom() <= scroll.bounds().bottom());
        window.click("complete-doc-uploaded-document", cx);
    });
    h.wait(cx, |v| {
        v.knowledge
            .documents
            .iter()
            .any(|d| d.id == "uploaded-document" && d.status != "pending")
    });
    assert_eq!(
        h.fixture
            .count("POST /api/v1/workspaces/workspace-ui/documents"),
        1
    );
    assert_eq!(h.fixture.count("PUT /uploads/uploaded-document"), 1);
    h.close(cx);
}

#[gpui_kit::test]
fn failed_upload_can_retry_without_creating_a_personal_attachment(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1280., 800.);
    enter(&h, cx);
    h.fixture
        .knowledge
        .lock()
        .unwrap()
        .fail_upload
        .store(true, Ordering::SeqCst);
    let file = std::path::PathBuf::from(std::env::var_os("CONTEXT_OS_DESKTOP_DATA_DIR").unwrap())
        .join("retry.txt");
    std::fs::write(&file, "retry upload bytes").unwrap();
    h.click(cx, "add-workspace-document");
    cx.simulate_path_prompt_response(move |_| Some(vec![file]));
    h.wait(cx, |v| {
        v.knowledge.uploads.values().any(|u| u.error.is_some())
    });
    let id = h
        .view
        .read_with(cx, |v, _| *v.knowledge.uploads.keys().next().unwrap());
    h.fixture
        .knowledge
        .lock()
        .unwrap()
        .fail_upload
        .store(false, Ordering::SeqCst);
    h.frame(cx, |window, cx| {
        window.click(
            gpui_kit::SharedString::from(format!("retry-upload-{id}")),
            cx,
        )
    });
    h.wait(cx, |v| {
        v.knowledge
            .documents
            .iter()
            .any(|d| d.id == "uploaded-document")
    });
    assert_eq!(
        h.fixture
            .count("DELETE /api/v1/documents/uploaded-document"),
        1
    );
    assert_eq!(
        h.fixture
            .count("POST /api/v1/workspaces/workspace-ui/documents"),
        2
    );
    assert_eq!(h.fixture.count("POST /api/v1/chat/sessions"), 0);
    h.close(cx);
}

#[gpui_kit::test]
fn narrow_workspace_panels_and_unsaved_note_leave_confirmation(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 768., 720.);
    enter(&h, cx);
    h.click(cx, "workspace-notes");
    edit(&h, cx, "note-content", "需要保护的笔记");
    h.frame(cx, |window, _| {
        assert!(window.find("save-note").visible());
        assert!(window.find("close-knowledge-panel").visible());
    });
    // The narrow view is now modal: explicitly dismiss it before using background navigation.
    h.click(cx, "close-knowledge-panel");
    h.click(cx, "workspaces-nav");
    h.click(cx, "accept-knowledge-confirm");
    h.wait(cx, |v| {
        v.knowledge.overview && v.knowledge.workspaces.len() == 2
    });
    h.click(cx, "workspace-workspace-two");
    h.wait(cx, |v| v.knowledge.documents.len() == 1);
    h.click(cx, "workspace-notes");
    h.view.read_with(cx, |v, cx| {
        assert!(v.knowledge.note_content.read(cx).value().is_empty());
        assert!(v.knowledge.panel == Some(Panel::Notes));
    });
    h.close(cx);
}

#[gpui_kit::test]
fn workspace_history_restores_citations_and_document_delete_is_confirmed(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1280., 800.);
    enter(&h, cx);
    h.click(cx, "workspace-history");
    h.wait(cx, |v| !v.loading && v.conversation.messages.len() == 1);
    h.click(cx, "citation-history-0-0");
    h.wait(cx, |v| {
        v.knowledge
            .preview
            .as_ref()
            .is_some_and(|(_, text)| !text.is_empty())
    });
    h.click(cx, "close-knowledge-panel");
    h.click(cx, "workspace-documents");
    h.click(cx, "delete-doc-ui-document");
    h.click(cx, "cancel-knowledge-confirm");
    assert_eq!(h.fixture.count("DELETE /api/v1/documents/ui-document"), 0);
    h.click(cx, "delete-doc-ui-document");
    h.click(cx, "accept-knowledge-confirm");
    h.wait(cx, |v| {
        v.knowledge.documents.iter().all(|d| d.id != "ui-document")
    });
    assert_eq!(h.fixture.count("DELETE /api/v1/documents/ui-document"), 1);
    h.close(cx);
}

#[gpui_kit::test]
fn delayed_refresh_cannot_erase_saved_note_or_replace_another_workspace(cx: &mut TestAppContext) {
    let h = Harness::new(cx, 1280., 800.);
    enter(&h, cx);
    h.fixture.hold_notes.store(true, Ordering::SeqCst);
    h.click(cx, "workspace-documents");
    h.click(cx, "refresh-materials");
    h.wait(cx, |_| h.fixture.waiting_notes.load(Ordering::SeqCst) == 1);
    h.click(cx, "workspace-notes");
    edit(&h, cx, "note-content", "读取发出后保存的笔记");
    h.click(cx, "save-note");
    h.wait(cx, |v| v.knowledge.note_id.is_some());
    h.fixture.hold_notes.store(false, Ordering::SeqCst);
    h.wait(cx, |v| !v.knowledge.busy(|_| true));
    h.view.read_with(cx, |v, _| {
        assert_eq!(v.knowledge.notes[0].content, "读取发出后保存的笔记")
    });
    // Hold another response from A, finish loading B, then deliver A's old response.
    h.fixture.hold_notes.store(true, Ordering::SeqCst);
    h.click(cx, "workspace-documents");
    h.click(cx, "refresh-materials");
    h.wait(cx, |_| h.fixture.waiting_notes.load(Ordering::SeqCst) == 1);
    h.click(cx, "workspaces-nav");
    h.wait(cx, |v| v.knowledge.workspaces.len() == 2);
    h.click(cx, "workspace-workspace-two");
    h.wait(cx, |v| v.knowledge.documents.len() == 1);
    h.fixture.hold_notes.store(false, Ordering::SeqCst);
    h.wait(cx, |v| v.knowledge.pending.is_empty());
    h.view.read_with(cx, |v, _| {
        assert_eq!(v.knowledge.documents[0].id, "other-document");
        assert!(v.knowledge.notes.is_empty());
    });
    h.close(cx);
}
