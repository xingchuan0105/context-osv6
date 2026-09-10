use super::*;
use contracts::{
    documents::Document,
    workspaces::{Workspace, WorkspaceNote},
};
use desktop_gpui::workspace::{Action, Reply, ResultData};
use std::collections::{BTreeSet, HashMap};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Panel {
    Documents,
    Notes,
    Preview,
}

#[derive(Clone)]
pub(super) enum Destination {
    Personal,
    Overview,
    Workspace(Workspace),
    Note(Option<WorkspaceNote>),
    Exit,
}

pub(super) enum Confirm {
    Leave(Destination),
    Delete(Action),
}

pub(super) struct Pending {
    epoch: u64,
    revision: u64,
    action: Action,
}
#[derive(Clone)]
pub(super) struct Upload {
    pub workspace: String,
    pub path: std::path::PathBuf,
    pub running: bool,
    pub error: Option<String>,
    pub document_id: Option<String>,
}

pub(super) struct Knowledge {
    pub overview: bool,
    pub active: Option<Workspace>,
    pub workspaces: Vec<Workspace>,
    pub documents: Vec<Document>,
    pub notes: Vec<WorkspaceNote>,
    pub selected: BTreeSet<String>,
    pub panel: Option<Panel>,
    pub name: Entity<TextareaState>,
    pub note_title: Entity<TextareaState>,
    pub note_content: Entity<TextareaState>,
    pub note_id: Option<String>,
    pub baseline: (String, String),
    pub preview: Option<(String, String)>,
    pub error: Option<String>,
    pub confirm: Option<Confirm>,
    pub uploads: HashMap<u64, Upload>,
    pub pending: HashMap<u64, Pending>,
    epoch: u64,
    serial: u64,
    revision: u64,
    pub saved: HashMap<Option<String>, (Conversation, String)>,
}

impl Knowledge {
    pub fn new(window: &mut Window, cx: &mut Context<ChatApp>) -> Self {
        Self {
            overview: false,
            active: None,
            workspaces: vec![],
            documents: vec![],
            notes: vec![],
            selected: BTreeSet::new(),
            panel: None,
            name: cx.new(|cx| {
                TextareaState::new(window, cx)
                    .rows(1)
                    .placeholder("工作区名称")
            }),
            note_title: cx.new(|cx| {
                TextareaState::new(window, cx)
                    .rows(1)
                    .placeholder("笔记标题")
            }),
            note_content: cx.new(|cx| {
                TextareaState::new(window, cx)
                    .rows(8)
                    .placeholder("记录你的想法…")
            }),
            note_id: None,
            baseline: Default::default(),
            preview: None,
            error: None,
            confirm: None,
            uploads: HashMap::new(),
            pending: HashMap::new(),
            epoch: 0,
            serial: 0,
            revision: 0,
            saved: HashMap::new(),
        }
    }
    pub fn scope(&self) -> Option<String> {
        self.active.as_ref().map(|w| w.id.clone())
    }
    pub fn dirty(&self, cx: &App) -> bool {
        self.note_title.read(cx).value().as_ref() != self.baseline.0
            || self.note_content.read(cx).value().as_ref() != self.baseline.1
    }
    pub fn busy(&self, matches: impl Fn(&Action) -> bool) -> bool {
        self.pending
            .values()
            .any(|p| p.epoch == self.epoch && matches(&p.action))
    }
}

impl ChatApp {
    pub(super) fn knowledge_action(&mut self, action: Action, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            self.knowledge.error = Some("请先连接本机服务".into());
            return;
        };
        if self.knowledge.busy(|pending| {
            matches!((pending, &action), (Action::CompleteUpload(a), Action::CompleteUpload(b)) if a == b) || matches!(
                (pending, &action),
                (Action::Load, Action::Load)
                    | (Action::Create(_), Action::Create(_))
                    | (Action::SaveNote { .. }, Action::SaveNote { .. })
            )
        }) {
            return;
        }
        self.knowledge.serial += 1;
        let request = self.knowledge.serial;
        let scope = self.knowledge.scope();
        if let (Action::Upload(path), Some(workspace)) = (&action, &scope) {
            self.knowledge.uploads.insert(
                request,
                Upload {
                    workspace: workspace.clone(),
                    path: path.clone(),
                    running: true,
                    error: None,
                    document_id: None,
                },
            );
        }
        if let Action::CompleteUpload(document_id) = &action {
            let previous = self.knowledge.uploads.iter().find_map(|(id, upload)| {
                (Some(&upload.workspace) == scope.as_ref()
                    && upload.document_id.as_ref() == Some(document_id))
                .then_some(*id)
            });
            if let Some(mut upload) = previous.and_then(|id| self.knowledge.uploads.remove(&id)) {
                upload.running = true;
                upload.error = None;
                self.knowledge.uploads.insert(request, upload);
            }
        }
        if !matches!(action, Action::Load) {
            self.knowledge.error = None;
        }
        self.knowledge.pending.insert(
            request,
            Pending {
                epoch: self.knowledge.epoch,
                revision: self.knowledge.revision,
                action: action.clone(),
            },
        );
        self.host.workspace(token, scope, request, action);
        cx.notify();
    }

    pub(super) fn apply_knowledge(
        &mut self,
        reply: Reply,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pending) = self.knowledge.pending.remove(&reply.request) else {
            return;
        };
        if let Some(upload) = self.knowledge.uploads.get_mut(&reply.request) {
            upload.running = false;
            upload.error = reply.result.as_ref().err().cloned();
            if let Ok(ResultData::Uploaded {
                document_id,
                completion,
            }) = &reply.result
            {
                upload.document_id = Some(document_id.clone());
                upload.error = completion.as_ref().err().cloned();
            }
        }
        let current =
            pending.epoch == self.knowledge.epoch && reply.workspace == self.knowledge.scope();
        // A late upload still belongs to its original workspace; only refresh that visible scope.
        if !current && !matches!(reply.action, Action::List | Action::Create(_)) {
            if matches!(reply.action, Action::Upload(_) | Action::CompleteUpload(_))
                && reply.workspace == self.knowledge.scope()
            {
                self.knowledge_action(Action::Load, cx);
            }
            return;
        }
        if current
            && matches!(reply.action, Action::Load)
            && pending.revision != self.knowledge.revision
        {
            // A read begun before a successful write cannot roll the visible collection back.
            self.knowledge_action(Action::Load, cx);
            return;
        }
        match reply.result {
            Err(error) => {
                if current || self.knowledge.overview {
                    self.knowledge.error = Some(error);
                }
            }
            Ok(ResultData::List(list)) => self.knowledge.workspaces = list,
            Ok(ResultData::Created(workspace)) => {
                self.knowledge.workspaces.insert(0, workspace.clone());
                self.knowledge
                    .name
                    .update(cx, |input, cx| input.set_value("", window, cx));
                if self.knowledge.overview {
                    self.navigate(Destination::Workspace(workspace), window, cx);
                }
            }
            Ok(ResultData::Loaded { documents, notes }) => {
                self.knowledge.selected.retain(|id| {
                    documents
                        .iter()
                        .any(|d| &d.id == id && d.status == "completed")
                });
                self.knowledge.documents = documents;
                self.knowledge.notes = notes;
            }
            Ok(ResultData::NoteSaved(note)) => {
                self.knowledge.revision += 1;
                if let Action::SaveNote { title, content, .. } = reply.action {
                    self.knowledge.baseline = (title, content);
                    self.knowledge.note_id = Some(note.id.clone());
                }
                self.knowledge.notes.retain(|n| n.id != note.id);
                self.knowledge.notes.insert(0, note);
                self.notice = "笔记已保存".into();
            }
            Ok(ResultData::Changed) => {
                self.knowledge.revision += 1;
                if let Action::DeleteDocument(id) = &reply.action {
                    self.knowledge
                        .uploads
                        .retain(|_, upload| upload.document_id.as_ref() != Some(id));
                }
                if matches!(reply.action, Action::DeleteNote(_)) {
                    self.set_note(None, window, cx);
                }
                self.knowledge_action(Action::Load, cx);
            }
            Ok(ResultData::Uploaded { completion, .. }) => {
                self.knowledge.revision += 1;
                self.knowledge.error = completion.err();
                self.knowledge_action(Action::Load, cx);
            }
            Ok(ResultData::Preview {
                document_id,
                content,
            }) => {
                if self
                    .knowledge
                    .preview
                    .as_ref()
                    .is_some_and(|(id, _)| *id == document_id)
                {
                    self.knowledge.preview = Some((document_id, content));
                }
            }
        }
    }

    pub(super) fn navigate(
        &mut self,
        destination: Destination,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(destination, Destination::Note(_))
            && self
                .knowledge
                .busy(|a| matches!(a, Action::SaveNote { .. } | Action::DeleteNote(_)))
        {
            return;
        }
        if self.knowledge.dirty(cx) {
            self.knowledge.confirm = Some(Confirm::Leave(destination));
            cx.notify();
            return;
        }
        self.navigate_now(destination, window, cx);
    }

    pub(super) fn navigate_now(
        &mut self,
        destination: Destination,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.knowledge.confirm = None;
        if let Destination::Note(note) = destination {
            self.set_note(note, window, cx);
            return;
        }
        if matches!(destination, Destination::Exit) {
            self.set_note(None, window, cx);
            self.request_exit(cx);
            return;
        }
        self.stop();
        let next_generation = self.conversation.generation + 1;
        let previous_scope = self.knowledge.scope();
        let draft = self.input.read(cx).value().to_string();
        self.knowledge.saved.insert(
            previous_scope,
            (std::mem::take(&mut self.conversation), draft),
        );
        self.knowledge.epoch += 1;
        self.knowledge.overview = matches!(destination, Destination::Overview);
        self.knowledge.active = if let Destination::Workspace(workspace) = destination {
            Some(workspace)
        } else {
            None
        };
        let scope = self.knowledge.scope();
        let (conversation, draft) = self.knowledge.saved.remove(&scope).unwrap_or_default();
        self.conversation = conversation;
        self.conversation.generation = next_generation;
        self.input
            .update(cx, |input, cx| input.set_value(draft, window, cx));
        self.set_note(None, window, cx);
        self.knowledge.panel = None;
        self.knowledge.documents.clear();
        self.knowledge.notes.clear();
        self.knowledge.selected.clear();
        self.knowledge.error = None;
        self.knowledge.preview = None;
        self.sessions.clear();
        self.loading = false;
        self.show_services = false;
        self.title_error = None;
        if let Some(token) = self.token.clone() {
            self.host.sessions(token, scope.clone());
        }
        if scope.is_some() {
            self.knowledge_action(Action::Load, cx);
        }
        if self.knowledge.overview {
            self.knowledge_action(Action::List, cx);
        }
        cx.notify();
    }

    pub(super) fn set_note(
        &mut self,
        note: Option<WorkspaceNote>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.knowledge.note_id = note.as_ref().map(|n| n.id.clone());
        let title = note.as_ref().map(|n| n.title.clone()).unwrap_or_default();
        let content = note.map(|n| n.content).unwrap_or_default();
        self.knowledge
            .note_title
            .update(cx, |v, cx| v.set_value(title.clone(), window, cx));
        self.knowledge
            .note_content
            .update(cx, |v, cx| v.set_value(content.clone(), window, cx));
        self.knowledge.baseline = (title, content);
        cx.notify();
    }

    pub(super) fn pick_document(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace) = self.knowledge.scope() else {
            return;
        };
        let selected = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("添加工作区资料".into()),
        });
        cx.spawn_in(window, async move |this, cx| match selected.await {
            Ok(Ok(Some(paths))) => {
                let _ = this.update_in(cx, |this, _, cx| {
                    if this.knowledge.scope().as_deref() != Some(&workspace) {
                        return;
                    }
                    if let Some(path) = paths.into_iter().next() {
                        this.knowledge.panel = Some(Panel::Documents);
                        this.knowledge_action(Action::Upload(path), cx);
                    }
                });
            }
            Ok(Err(error)) => {
                let _ = this.update_in(cx, |this, _, cx| {
                    this.knowledge.error = Some(error.to_string());
                    cx.notify();
                });
            }
            _ => {}
        })
        .detach();
    }

    pub(super) fn preview_document(&mut self, id: String, cx: &mut Context<Self>) {
        self.knowledge.panel = Some(Panel::Preview);
        self.knowledge.preview = Some((id.clone(), String::new()));
        self.knowledge_action(Action::Preview(id), cx);
    }

    pub(super) fn poll_documents(&mut self, cx: &mut Context<Self>) {
        if self.token.is_some()
            && self.knowledge.active.is_some()
            && !self.services.busy()
            && self.knowledge.documents.iter().any(|d| {
                matches!(
                    d.status.as_str(),
                    "enqueueing" | "queued" | "processing" | "deleting"
                )
            })
        {
            self.knowledge_action(Action::Load, cx);
        }
    }
}
