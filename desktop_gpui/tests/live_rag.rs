//! Real Host + isolated API/worker/PG/Redis + configured providers. Opt-in only.
use contracts::chat::ChatEvent;
use desktop_gpui::{
    runtime::{Host, Update},
    session::Conversation,
    workspace::{Action, ResultData},
};
use futures::{StreamExt, channel::mpsc::UnboundedReceiver};
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, String>;
fn check(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(message.into()) }
}

struct Journey {
    root: PathBuf,
    host: Host,
    events: UnboundedReceiver<Update>,
    token: String,
    user: String,
    workspace: Option<String>,
    serial: u64,
    documents: Vec<String>,
    sessions: Vec<String>,
    steps: Vec<String>,
}

impl Journey {
    fn save(&self, name: &str, value: Value) -> Result<()> {
        fs::write(
            self.root.join(name),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .map_err(|e| e.to_string())
    }
    fn record(&mut self, step: impl Into<String>) -> Result<()> {
        let step = step.into();
        println!("PASS {step}");
        self.steps.push(step);
        self.save("steps.json", json!(self.steps))
    }
    async fn next(&mut self, seconds: u64) -> Result<Update> {
        tokio::time::timeout(Duration::from_secs(seconds), self.events.next())
            .await
            .map_err(|_| "Host update timed out")?
            .ok_or_else(|| "Host channel closed".into())
    }
    async fn action(&mut self, action: Action) -> Result<ResultData> {
        self.serial += 1;
        self.host.workspace(
            self.token.clone(),
            self.workspace.clone(),
            self.serial,
            action,
        );
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            check(Instant::now() < deadline, "workspace request timed out")?;
            if let Update::Workspace(reply) = self.next(90).await? {
                if reply.request == self.serial {
                    check(
                        reply.workspace == self.workspace,
                        "workspace reply scope changed",
                    )?;
                    return reply.result;
                }
            }
        }
    }
    async fn raw(&self, method: &str, path: String) -> Result<Value> {
        desktop_gpui::workspace::call(&self.token, method, path, None).await
    }
    fn sql(&self, query: &str) -> Result<Value> {
        // Read-only queries on the test's own fresh database; no funding/bypass.
        let mut command =
            Command::new(PathBuf::from(std::env::var_os("PG_BIN_DIR").unwrap()).join("psql.exe"));
        command.args([
            "-X",
            "-h",
            "127.0.0.1",
            "-p",
            &std::env::var("CLIENT_PG_PORT").unwrap(),
            "-U",
            "avrag",
            "-d",
            "avrag_client",
            "-v",
            "ON_ERROR_STOP=1",
            "-tAc",
            query,
        ]);
        command.env(
            "PGOPTIONS",
            format!("-c statement_timeout=10000 -c default_transaction_read_only=on -c app.current_user={}", self.user),
        );
        desktop_core::win_cmd::hide_console(&mut command);
        let output = command.output().map_err(|e| e.to_string())?;
        check(
            output.status.success(),
            &String::from_utf8_lossy(&output.stderr),
        )?;
        serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())
    }
    async fn run(&mut self) -> Result<()> {
        self.host.login(self.root.join("app-data"));
        loop {
            if let Update::Login(result) = self.next(210).await? {
                let session = result?;
                check(session.ready, "local session not ready")?;
                self.token = session.token.ok_or("missing local token")?;
                self.user = session.user.ok_or("missing local user")?.id;
                uuid::Uuid::parse_str(&self.user).map_err(|e| e.to_string())?;
                break;
            }
        }
        self.record("isolated native stack and local authentication")?;
        let deadline = Instant::now() + Duration::from_secs(35);
        while !fs::read_to_string(self.root.join("logs/worker.log"))
            .unwrap_or_default()
            .contains("worker heartbeat")
        {
            check(
                Instant::now() < deadline,
                "worker initialization did not finish",
            )?;
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        let wallet = self.raw("GET", "/api/v1/billing/wallet".into()).await?;
        self.save("wallet-before.json", wallet.clone())?;
        check(
            wallet["ok"] == true && wallet["data"]["balance_fen"].as_i64().unwrap_or(0) > 0,
            "new local account has no signup balance",
        )?;
        self.record("worker ready and signup wallet available")?;
        match self
            .action(Action::Create(format!(
                "GPUI Office RAG {}",
                uuid::Uuid::new_v4()
            )))
            .await?
        {
            ResultData::Created(workspace) => self.workspace = Some(workspace.id),
            _ => return Err("unexpected workspace result".into()),
        }
        self.record("workspace created through GPUI Host")?;
        let cases = [
            (
                "phase0-mini.docx",
                "docx",
                vec!["LiteParse"],
                vec!["Context-OS"],
            ),
            (
                "inventory.xlsx",
                "xlsx",
                vec!["Cedar", "137"],
                vec!["Cedar", "137"],
            ),
            (
                "delivery.pptx",
                "pptx",
                vec!["Violet Harbor"],
                vec!["Violet Harbor"],
            ),
        ];
        // Ingest all formats before any answer, so doc_scope excludes real other documents.
        for (filename, _, _, preview_markers) in &cases {
            let id = match self
                .action(Action::Upload(self.root.join("fixtures").join(filename)))
                .await?
            {
                ResultData::Uploaded {
                    document_id,
                    completion,
                } => {
                    self.documents.push(document_id.clone());
                    completion?;
                    document_id
                }
                _ => return Err("unexpected upload result".into()),
            };
            let deadline = Instant::now() + Duration::from_secs(360);
            let mut states: Vec<Value> = vec![];
            loop {
                let product = tokio::task::spawn_blocking(
                    desktop_core::local_product::get_local_product_status,
                )
                .await
                .map_err(|e| e.to_string())?;
                check(
                    product.worker_ok,
                    "worker exited during ingestion; see worker log and Windows crash record",
                )?;
                check(
                    !fs::read_to_string(self.root.join("logs/worker.log"))
                        .unwrap_or_default()
                        .contains("document ir validation failed"),
                    "document IR rejected; stop before retrying an unchanged parse result",
                )?;
                check(
                    Instant::now() < deadline,
                    "document ingestion exceeded six minutes",
                )?;
                if let ResultData::Loaded { documents, .. } = self.action(Action::Load).await? {
                    let doc = documents
                        .iter()
                        .find(|d| d.id == id)
                        .ok_or("uploaded document missing")?;
                    if states.last().and_then(|s| s["status"].as_str()) != Some(doc.status.as_str())
                    {
                        states.push(json!({"status":doc.status,"chunks":doc.chunk_count}));
                        self.save(&format!("{filename}.states.json"), json!(states))?;
                        println!("DOCUMENT {filename}: {}", doc.status);
                    }
                    check(
                        !matches!(doc.status.as_str(), "failed" | "upload_invalid"),
                        "ingestion failed; see worker log",
                    )?;
                    if doc.status == "completed" {
                        check(doc.chunk_count > 0, "completed document has no chunks")?;
                        break;
                    }
                }
                // Load fetches several endpoints; stay below the API's 60/min limit.
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            match self.action(Action::Preview(id)).await? {
                ResultData::Preview { content, .. } => {
                    fs::write(self.root.join(format!("{filename}.preview.md")), &content)
                        .map_err(|e| e.to_string())?;
                    check(
                        preview_markers.iter().all(|m| content.contains(m)),
                        "stored preview lost fixture facts",
                    )?;
                }
                _ => return Err("unexpected document preview".into()),
            }
            self.record(format!(
                "{filename}: signed upload, worker completion, chunks and preview"
            ))?;
            uuid::Uuid::parse_str(self.documents.last().unwrap()).map_err(|e| e.to_string())?;
            // Short documents can legitimately produce zero TOC entries. Compare
            // the producer's actual count with persisted rows, under the same owner.
            let worker_log =
                fs::read_to_string(self.root.join("logs/worker.log")).map_err(|e| e.to_string())?;
            let document_marker = format!("document_id={}", self.documents.last().unwrap());
            let expected_toc = worker_log
                .lines()
                .filter(|line| {
                    line.contains(&document_marker)
                        && line.contains("windowed profile+summary+triplet done")
                })
                .filter_map(|line| {
                    line.split_whitespace().find_map(|field| {
                        field
                            .strip_prefix("toc=")
                            .and_then(|value| value.parse::<usize>().ok())
                    })
                })
                .next_back()
                .ok_or("missing TOC production count")?;
            let toc = self.sql(&format!(
                "SELECT COALESCE(jsonb_agg(jsonb_build_object('title',title,'owner_user_id',owner_user_id)),'[]') FROM document_toc WHERE document_id='{}'",
                self.documents.last().unwrap()
            ))?;
            self.save(&format!("{filename}.toc.json"), toc.clone())?;
            check(
                toc.as_array().is_some_and(|rows| {
                    rows.len() == expected_toc
                        && rows.iter().all(|row| row["owner_user_id"] == self.user)
                }),
                "document TOC count differs from produced entries or has wrong owner",
            )?;
            self.record(format!(
                "{filename}: {expected_toc} produced TOC entries read back"
            ))?;
        }
        for (index, (_, kind, expected, _)) in cases.iter().enumerate() {
            self.ask(kind, self.documents[index].clone(), expected)
                .await?;
        }
        self.billing().await?;
        Ok(())
    }
    async fn ask(&mut self, kind: &str, document: String, expected: &[&str]) -> Result<()> {
        let query = fs::read_to_string(
            self.root
                .join(format!("bin/prompts/eval/gpui-office/{kind}.md")),
        )
        .map_err(|e| e.to_string())?;
        let mut conversation = Conversation::default();
        let generation = conversation.begin(query.clone()).unwrap();
        let cancel = self.host.chat(
            self.token.clone(),
            query,
            None,
            self.workspace.clone(),
            vec![document.clone()],
            generation,
        );
        let mut events = vec![];
        let result = tokio::time::timeout(Duration::from_secs(240), async {
            loop {
                match self.next(240).await? {
                    Update::Event(g, event) if g == generation => {
                        events.push(serde_json::to_value(&event).unwrap());
                        if let ChatEvent::Error { message, .. } = &event {
                            return Err(message.clone());
                        }
                        conversation.event(g, event);
                    }
                    Update::End(g, result) if g == generation => return result,
                    _ => {}
                }
            }
        })
        .await
        .unwrap_or_else(|_| Err("RAG exceeded four minutes".into()));
        cancel.cancel();
        if let Some(session) = &conversation.session_id {
            self.sessions.push(session.clone());
        }
        self.save(&format!("{kind}.events.json"), json!(events))?;
        self.save(&format!("{kind}.answer.json"), json!({"document":document,"session":conversation.session_id,"request":conversation.turn.request_id,"answer":conversation.turn.answer_text,"citations":conversation.turn.citations,"error":result.as_ref().err()}))?;
        result?;
        check(
            conversation.turn.status == web_sdk::TurnStatus::Done,
            "RAG did not finish normally",
        )?;
        check(
            expected
                .iter()
                .all(|m| conversation.turn.answer_text.contains(m)),
            "answer missed fixture facts",
        )?;
        check(
            !conversation.turn.citations.is_empty(),
            "RAG returned no citations",
        )?;
        check(
            conversation
                .turn
                .citations
                .iter()
                .all(|c| web_sdk::CitationView::from_value(c).doc_id == document),
            "citation escaped selected document",
        )?;
        self.record(format!(
            "{kind}: real RAG answer and selected-document citations"
        ))
    }
    async fn billing(&mut self) -> Result<()> {
        // Allow asynchronous observers to finish; no additional model request.
        tokio::time::sleep(Duration::from_secs(2)).await;
        let usage = self.sql("SELECT COALESCE(jsonb_agg(jsonb_build_object('user_id',user_id,'feature',feature,'stage',stage,'usage_kind',usage_kind,'billable',billable,'credential_source',credential_source,'total_tokens',total_tokens,'request_id',request_id,'session_id',session_id) ORDER BY created_at),'[]') FROM llm_usage_events")?;
        self.save("usage.json", usage.clone())?;
        let rows = usage.as_array().ok_or("usage rows are not an array")?;
        check(
            rows.iter()
                .any(|r| r["stage"] == "embedding" && r["total_tokens"].as_i64().unwrap_or(0) > 0),
            "no embedding usage recorded",
        )?;
        check(
            rows.iter().all(|r| r["user_id"] == self.user),
            "usage has wrong account ownership",
        )?;
        let ledger = self.sql("SELECT COALESCE(jsonb_agg(jsonb_build_object('kind',kind,'amount_fen',amount_fen,'metadata',metadata) ORDER BY created_at),'[]') FROM wallet_ledger")?;
        self.save("wallet-ledger.json", ledger.clone())?;
        check(
            ledger
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["kind"] == "usage_debit" && r["amount_fen"].as_i64().unwrap_or(0) < 0),
            "no real usage debit recorded",
        )?;
        let audits = self.sql("SELECT COALESCE(jsonb_agg(jsonb_build_object('actor_id',actor_id,'action',action,'resource_id',resource_id,'payload',payload) ORDER BY created_at),'[]') FROM audit_log WHERE resource_type='chat'")?;
        self.save("audit.json", audits.clone())?;
        check(
            self.sessions.iter().all(|s| {
                audits.as_array().unwrap().iter().any(|a| {
                    a["resource_id"] == *s
                        && a["payload"]["mode"] == "rag"
                        && a["actor_id"] == self.user
                })
            }),
            "missing or wrong-scope RAG audit",
        )?;
        self.save(
            "wallet-after.json",
            self.raw("GET", "/api/v1/billing/wallet".into()).await?,
        )?;
        self.record("embedding metering, usage debit and RAG audits read back")
    }
    async fn cleanup_call(&self, method: &str, path: String) -> Result<Value> {
        match self.raw(method, path.clone()).await {
            Err(error) if error.starts_with("429 ") => {
                println!("CLEANUP rate limited; waiting one request window before retry");
                tokio::time::sleep(Duration::from_secs(61)).await;
                self.raw(method, path).await
            }
            result => result,
        }
    }
    async fn cleanup(&mut self) -> Vec<String> {
        let mut errors = vec![];
        for id in self.documents.clone() {
            if let Err(e) = self
                .cleanup_call("DELETE", web_sdk::workspace_api::document_url("", &id))
                .await
            {
                errors.push(e);
            }
        }
        if let Some(id) = &self.workspace {
            if let Err(e) = self
                .cleanup_call("DELETE", web_sdk::workspace_api::workspace_url("", id))
                .await
            {
                errors.push(e);
            }
            match self
                .cleanup_call("GET", web_sdk::workspace_api::workspaces_url(""))
                .await
            {
                Ok(value) => match web_sdk::workspace_api::parse_workspace_list(
                    &serde_json::to_vec(&value).unwrap(),
                ) {
                    Ok(list) if list.workspaces.iter().all(|w| &w.id != id) => {}
                    _ => errors.push("test workspace removal not verified".into()),
                },
                Err(e) => errors.push(e),
            }
        }
        if !self.documents.is_empty() {
            match self.cleanup_call("GET", "/api/v1/documents".into()).await {
                Ok(value) => match web_sdk::workspace_api::parse_workspace_documents(
                    &serde_json::to_vec(&value).unwrap(),
                ) {
                    Ok(list)
                        if self
                            .documents
                            .iter()
                            .all(|id| list.documents.iter().all(|d| &d.id != id)) => {}
                    _ => errors.push("test document removal not verified".into()),
                },
                Err(e) => errors.push(e),
            }
        }
        errors
    }
}

#[test]
#[ignore = "paid provider calls in isolated native stack; accept-managed.ps1 -OfficeRag"]
fn office_ingestion_and_scoped_rag() {
    assert_eq!(
        std::env::var("GPUI_ACCEPTANCE_OFFICE_RAG").as_deref(),
        Ok("1")
    );
    assert!(std::env::var_os("CLIENT_API_BASE_URL").is_none());
    let root = PathBuf::from(std::env::var_os("CONTEXT_OS_CLIENT_HOME").unwrap());
    assert!(root.join("acceptance.marker").is_file() && !root.join("data").exists());
    assert!(
        root.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("gpui-managed-")
    );
    let (host, events) = Host::new().unwrap();
    let mut journey = Journey {
        root: root.clone(),
        host,
        events,
        token: String::new(),
        user: String::new(),
        workspace: None,
        serial: 0,
        documents: vec![],
        sessions: vec![],
        steps: vec![],
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let result = runtime.block_on(journey.run());
    let cleanup = runtime.block_on(journey.cleanup());
    journey.save("journey.json", json!({"ok":result.is_ok() && cleanup.is_empty(),"error":result.err(),"cleanup_errors":cleanup,"steps":journey.steps,"documents":journey.documents,"workspace":journey.workspace,"sessions":journey.sessions})).unwrap();
    // Always drop outside the runtime; Host's own guard releases only this stack.
    drop(journey);
    let report: Value =
        serde_json::from_slice(&fs::read(root.join("journey.json")).unwrap()).unwrap();
    assert_eq!(
        report["ok"], true,
        "real Office/RAG gate failed; see journey.json"
    );
}
