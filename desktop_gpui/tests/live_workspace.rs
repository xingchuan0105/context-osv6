//! Opt-in real local API contract journey. No desktop, ingestion or model request.
use desktop_gpui::{
    runtime::{Host, Update},
    workspace::{self, Action, ResultData},
};
use futures::{StreamExt, channel::mpsc::UnboundedReceiver};
use serde_json::{Value, json};
use std::{path::PathBuf, time::Duration};
use web_sdk::workspace_api as api;

struct Journey {
    host: Host,
    updates: UnboundedReceiver<Update>,
    token: String,
    serial: u64,
    workspaces: Vec<String>,
    notes: Vec<(String, String)>,
    documents: Vec<String>,
    steps: Vec<&'static str>,
}

fn ensure(ok: bool, message: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(message.into()) }
}

impl Journey {
    async fn action(&mut self, scope: Option<&str>, action: Action) -> Result<ResultData, String> {
        self.serial += 1;
        self.host.workspace(
            self.token.clone(),
            scope.map(str::to_string),
            self.serial,
            action,
        );
        tokio::time::timeout(Duration::from_secs(70), async {
            while let Some(update) = self.updates.next().await {
                if let Update::Workspace(reply) = update {
                    if reply.request == self.serial {
                        ensure(
                            reply.workspace.as_deref() == scope,
                            "Host reply scope changed",
                        )?;
                        return reply.result;
                    }
                }
            }
            Err("Host closed without a workspace result".into())
        })
        .await
        .map_err(|_| "Host workspace request timed out".to_string())?
    }

    async fn raw(
        &self,
        method: &str,
        url: String,
        payload: Option<Value>,
    ) -> Result<Value, String> {
        workspace::call(&self.token, method, url, payload).await
    }

    async fn create(&mut self, name: String) -> Result<String, String> {
        match self.action(None, Action::Create(name.clone())).await? {
            ResultData::Created(workspace) => {
                let id = workspace.id;
                self.workspaces.push(id.clone());
                ensure(
                    workspace.name == name,
                    "workspace name changed in the real API",
                )?;
                Ok(id)
            }
            _ => Err("unexpected workspace create result".into()),
        }
    }

    async fn run(&mut self) -> Result<(), String> {
        // Use the existing session, without renewing credentials or starting any service.
        self.raw("GET", "/api/auth/me".into(), None).await?;
        self.steps.push("existing session authenticated");
        let marker = uuid::Uuid::new_v4().to_string();
        let first = self.create(format!("GPUI D3 自动验收 A {marker}")).await?;
        let second = self.create(format!("GPUI D3 自动验收 B {marker}")).await?;
        match self.action(None, Action::List).await? {
            ResultData::List(list) => ensure(
                self.workspaces
                    .iter()
                    .all(|id| list.iter().any(|w| &w.id == id)),
                "created workspace missing from list",
            )?,
            _ => return Err("unexpected workspace list result".into()),
        }
        self.steps.push("workspace create and list readback");
        match self.action(Some(&first), Action::Load).await? {
            ResultData::Loaded { documents, notes } => ensure(
                documents.is_empty() && notes.is_empty(),
                "new workspace was not empty",
            )?,
            _ => return Err("unexpected initial load result".into()),
        }
        self.steps
            .push("new workspace has no foreign documents or notes");
        let title = "原生笔记验收".to_string();
        let content = "第一行中文\n第二行：保存、回读、修改。".to_string();
        let note = match self
            .action(
                Some(&first),
                Action::SaveNote {
                    id: None,
                    title: title.clone(),
                    content: content.clone(),
                },
            )
            .await?
        {
            ResultData::NoteSaved(note) => note,
            _ => return Err("unexpected note create result".into()),
        };
        self.notes.push((first.clone(), note.id.clone()));
        ensure(
            note.title == title && note.content == content,
            "note create response lost content",
        )?;
        match self.action(Some(&first), Action::Load).await? {
            ResultData::Loaded { notes, documents } => {
                ensure(
                    notes
                        .iter()
                        .any(|n| n.id == note.id && n.content == content),
                    "note did not persist",
                )?;
                ensure(
                    documents.is_empty(),
                    "saving a note unexpectedly created a retrieval document",
                )?;
            }
            _ => return Err("unexpected note readback result".into()),
        }
        self.steps.push("note create readback without indexing");
        let changed = format!("{content}\n已修改。");
        self.action(
            Some(&first),
            Action::SaveNote {
                id: Some(note.id.clone()),
                title: title.clone(),
                content: changed.clone(),
            },
        )
        .await?;
        ensure(
            self.action(
                Some(&second),
                Action::SaveNote {
                    id: Some(note.id.clone()),
                    title,
                    content: "wrong workspace".into(),
                },
            )
            .await
            .is_err(),
            "cross-workspace note update was accepted",
        )?;
        match self.action(Some(&first), Action::Load).await? {
            ResultData::Loaded { notes, .. } => ensure(
                notes
                    .iter()
                    .any(|n| n.id == note.id && n.content == changed),
                "note update or isolation failed",
            )?,
            _ => return Err("unexpected updated note result".into()),
        }
        match self.action(Some(&second), Action::Load).await? {
            ResultData::Loaded { notes, documents } => ensure(
                notes.is_empty() && documents.is_empty(),
                "second workspace contains first workspace data",
            )?,
            _ => return Err("unexpected second workspace result".into()),
        }
        self.steps.push("note update and cross-workspace isolation");
        self.action(Some(&first), Action::DeleteNote(note.id.clone()))
            .await?;
        self.notes.retain(|(_, id)| id != &note.id);
        match self.action(Some(&first), Action::Load).await? {
            ResultData::Loaded { notes, .. } => {
                ensure(notes.is_empty(), "deleted note still present")?
            }
            _ => return Err("unexpected deleted note readback".into()),
        }
        self.steps.push("note delete readback");
        // Metadata only: no PUT occurs. Missing object fails before task enqueue/model work.
        let value = self.raw("POST", api::workspace_document_upload_url("", &first), Some(json!({"filename":"gpui-missing-body.txt","file_size":19,"mime_type":"text/plain"}))).await?;
        let id = value
            .get("document_id")
            .and_then(Value::as_str)
            .ok_or("missing document ID")?
            .to_string();
        self.documents.push(id.clone());
        ensure(
            self.action(Some(&first), Action::CompleteUpload(id.clone()))
                .await
                .is_err(),
            "missing upload bytes were incorrectly accepted",
        )?;
        match self.action(Some(&first), Action::Load).await? {
            ResultData::Loaded { documents, .. } => ensure(
                documents
                    .iter()
                    .any(|d| d.id == id && d.status == "upload_invalid"),
                "missing bytes did not become upload_invalid",
            )?,
            _ => return Err("unexpected failed submit readback".into()),
        }
        self.steps
            .push("missing upload body rejected before ingestion");
        Ok(())
    }

    async fn cleanup(&mut self) -> Vec<String> {
        let mut errors = vec![];
        for (workspace, id) in self.notes.clone() {
            if let Err(error) = self
                .raw("DELETE", api::workspace_note_url("", &workspace, &id), None)
                .await
            {
                errors.push(format!("note {id}: {error}"));
            }
        }
        for id in &self.documents {
            if let Err(error) = self.raw("DELETE", api::document_url("", id), None).await {
                errors.push(format!("document {id}: {error}"));
            }
        }
        for id in &self.workspaces {
            if let Err(error) = self.raw("DELETE", api::workspace_url("", id), None).await {
                errors.push(format!("workspace {id}: {error}"));
            }
        }
        match self.action(None, Action::List).await {
            Ok(ResultData::List(list))
                if self
                    .workspaces
                    .iter()
                    .all(|id| list.iter().all(|w| &w.id != id)) => {}
            _ => errors.push("test workspace removal could not be verified".into()),
        }
        let documents = self
            .raw("GET", "/api/v1/documents".into(), None)
            .await
            .and_then(|value| {
                api::parse_workspace_documents(&serde_json::to_vec(&value).unwrap())
                    .map_err(|e| e.to_string())
            });
        match documents {
            Ok(list)
                if self
                    .documents
                    .iter()
                    .all(|id| list.documents.iter().all(|d| &d.id != id)) => {}
            _ => errors.push("test document removal could not be verified".into()),
        }
        if errors.is_empty() {
            self.steps
                .push("test objects deleted and workspace removal verified");
        }
        errors
    }
}

#[test]
#[ignore = "opt-in isolated live API writes; run accept-workspace-live.ps1"]
fn real_workspace_notes_and_invalid_upload() {
    assert_eq!(
        std::env::var("GPUI_ACCEPTANCE_LIVE_WORKSPACE").as_deref(),
        Ok("1")
    );
    assert_eq!(
        desktop_core::product_api_base_url(),
        "http://127.0.0.1:18082"
    );
    let session = PathBuf::from(std::env::var_os("CONTEXT_OS_DESKTOP_DATA_DIR").unwrap());
    assert_eq!(session.file_name().unwrap(), "session-current");
    let report = PathBuf::from(std::env::var_os("GPUI_ACCEPTANCE_REPORT_DIR").unwrap());
    let token =
        desktop_core::local_session_token(&session).expect("existing isolated session is required");
    let (host, updates) = Host::new().unwrap();
    let mut journey = Journey {
        host,
        updates,
        token,
        serial: 0,
        workspaces: vec![],
        notes: vec![],
        documents: vec![],
        steps: vec![],
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let result = runtime.block_on(journey.run());
    let cleanup = runtime.block_on(journey.cleanup());
    let ok = result.is_ok() && cleanup.is_empty();
    let proof = json!({"ok":ok,"steps":journey.steps,"error":result.err(),"cleanup_errors":cleanup,"workspaces":journey.workspaces,"documents":journey.documents,"coverage":"real GPUI Host and existing local API; metadata-only invalid upload; no desktop or model call"});
    std::fs::write(
        report.join("journey.json"),
        serde_json::to_vec_pretty(&proof).unwrap(),
    )
    .unwrap();
    assert!(ok, "live workspace gate failed; see journey.json");
    // Journey/Host are dropped outside block_on; their existing shutdown guard has no managed processes.
}
