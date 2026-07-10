//! HTTP client helpers on TestContext.

use std::sync::atomic::Ordering;
use std::time::Duration;

use super::super::{
    DocumentStatus, HttpResponse, WorkspaceInner, WorkspaceResponse, SseEvent, SseParser,
    UploadResponse,
    mock_servers::{
        set_mock_rag_codegen_chunk_id, set_mock_rag_codegen_chunk_ids, set_mock_rag_codegen_query,
    },
    setup,
};
use super::diagnostics::dump_ingestion_failure_diagnostics;
use super::TestContext;
use super::profiles::ChatStreamParams;

impl TestContext {
    pub async fn create_workspace(&self, name: &str) -> anyhow::Result<WorkspaceInner> {
        let resp = self
            .http_client
            .post(format!("{}/api/v1/workspaces", self.base_url))
            .json(&serde_json::json!({ "name": name, "description": "" }))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body = resp.json::<serde_json::Value>().await?;
        if status != 201 {
            anyhow::bail!("create notebook failed: HTTP {status}, body: {body}");
        }
        let wrapper: WorkspaceResponse = serde_json::from_value(body)?;
        Ok(wrapper.workspace)
    }

    pub async fn upload_document(&self, fixture: &str) -> anyhow::Result<UploadResponse> {
        let notebook = self.create_workspace("test-notebook").await?;
        self.upload_document_to_notebook(fixture, &notebook.id)
            .await
    }

    pub async fn upload_document_to_notebook(
        &self,
        fixture: &str,
        workspace_id: &str,
    ) -> anyhow::Result<UploadResponse> {
        let content = setup::load_fixture(fixture)?;
        self.upload_bytes_to_notebook(fixture, content.into_bytes(), workspace_id)
            .await
    }

    pub async fn upload_file_from_path_to_notebook(
        &self,
        file_path: &str,
        workspace_id: &str,
    ) -> anyhow::Result<UploadResponse> {
        let path = std::path::Path::new(file_path);
        let bytes =
            std::fs::read(path).map_err(|e| anyhow::anyhow!("read {}: {e}", path.display()))?;
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("document.bin");
        self.upload_bytes_to_notebook(filename, bytes, workspace_id)
            .await
    }

    pub async fn upload_file_from_path(&self, file_path: &str) -> anyhow::Result<UploadResponse> {
        let notebook = self.create_workspace("test-notebook").await?;
        self.upload_file_from_path_to_notebook(file_path, &notebook.id)
            .await
    }

    async fn upload_bytes_to_notebook(
        &self,
        filename: &str,
        bytes: Vec<u8>,
        workspace_id: &str,
    ) -> anyhow::Result<UploadResponse> {
        let mime_type = setup::mime_type_for_filename(filename);

        let resp = self
            .http_client
            .post(format!(
                "{}/api/v1/workspaces/{}/documents",
                self.base_url, workspace_id
            ))
            .json(&serde_json::json!({
                "filename": filename,
                "file_size": bytes.len(),
                "mime_type": mime_type,
            }))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body = resp.json::<serde_json::Value>().await?;
        if !(200..300).contains(&status) {
            anyhow::bail!("upload document failed: HTTP {status}, body: {body}");
        }

        let document_id = body["document_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing document_id in upload response: {body}"))?
            .to_string();

        let upload_resp = self
            .http_client
            .put(format!("{}/dev-upload/{document_id}", self.base_url))
            .body(bytes)
            .send()
            .await?;
        if !upload_resp.status().is_success() {
            let status = upload_resp.status().as_u16();
            let body = upload_resp.text().await.unwrap_or_default();
            anyhow::bail!("upload PUT failed: HTTP {status}, body: {body}");
        }
        self.verify_uploaded_object_readable(&document_id, workspace_id, filename)
            .await?;

        Ok(UploadResponse {
            document_id,
            workspace_id: workspace_id.to_string(),
            upload_url: String::new(),
            status,
        })
    }

    pub async fn wait_for_ingestion(
        &mut self,
        doc_id: &str,
        timeout: Duration,
    ) -> anyhow::Result<DocumentStatus> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut last_status = String::new();
        loop {
            if let Some(worker) = self.worker.as_mut()
                && let Ok(Some(status)) = worker.try_wait()
            {
                anyhow::bail!(
                    "worker process exited unexpectedly (status={status:?}) while waiting on doc={doc_id}, last status={last_status}"
                );
            }

            let body = self.fetch_status_with_retry(doc_id).await?;
            let status = body["status"].as_str().unwrap_or("unknown").to_string();
            if status != last_status {
                eprintln!("[wait_for_ingestion] doc={doc_id} status={status}");
                last_status = status.clone();
            }
            match status.as_str() {
                "completed" => return Ok(DocumentStatus::Completed),
                "failed" => {
                    dump_ingestion_failure_diagnostics(self, doc_id).await;
                    return Ok(DocumentStatus::Failed);
                }
                _ => {}
            }
            if tokio::time::Instant::now() > deadline {
                dump_ingestion_failure_diagnostics(self, doc_id).await;
                anyhow::bail!(
                    "wait_for_ingestion timed out after {timeout:?}, last status={last_status}"
                );
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    pub async fn fetch_document_status(&self, doc_id: &str) -> anyhow::Result<serde_json::Value> {
        self.fetch_status_with_retry(doc_id).await
    }

    pub fn worker_log_tail(&self, max_lines: usize) -> String {
        let Some(ref path) = self.worker_log_path else {
            return String::new();
        };
        let content = std::fs::read_to_string(path).unwrap_or_default();
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() <= max_lines {
            return content;
        }
        lines[lines.len() - max_lines..].join("\n")
    }

    async fn fetch_status_with_retry(&self, doc_id: &str) -> anyhow::Result<serde_json::Value> {
        const MAX_ATTEMPTS: u32 = 3;
        let url = format!("{}/api/v1/documents/{doc_id}/status", self.base_url);
        let mut last_err: Option<anyhow::Error> = None;
        for attempt in 1..=MAX_ATTEMPTS {
            match self.http_client.get(&url).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_server_error() {
                        last_err = Some(anyhow::anyhow!("server error HTTP {status}"));
                    } else if status.is_client_error() {
                        let body = resp.text().await.unwrap_or_default();
                        return Err(anyhow::anyhow!(
                            "client error fetching status: HTTP {status}, body: {body}"
                        ));
                    } else {
                        return Ok(resp.json::<serde_json::Value>().await?);
                    }
                }
                Err(e) => {
                    last_err = Some(anyhow::Error::from(e));
                }
            }
            if attempt < MAX_ATTEMPTS {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("fetch_status exhausted retries")))
    }

    async fn verify_uploaded_object_readable(
        &self,
        document_id: &str,
        workspace_id: &str,
        filename: &str,
    ) -> anyhow::Result<()> {
        let object_path = format!("{}/{}/{}/{}", self.org_id, workspace_id, document_id, filename);
        let object_full_path = std::path::Path::new(&self.object_root).join(&object_path);
        if !object_full_path.is_file() {
            anyhow::bail!(
                "dev-upload succeeded but object missing at {} (object_root={}, object_path={}); possible API/worker object store split (different AVRAG_OBJECT_ROOT)",
                object_full_path.display(),
                self.object_root,
                object_path
            );
        }
        Ok(())
    }

    pub async fn chat(
        &self,
        query: &str,
        workspace_id: &str,
        doc_scope: &[String],
    ) -> anyhow::Result<HttpResponse> {
        self.post_rag_chat(query, workspace_id, doc_scope, None, true)
            .await
    }

    pub async fn chat_without_mock_chunk_pin(
        &self,
        query: &str,
        workspace_id: &str,
        doc_scope: &[String],
    ) -> anyhow::Result<HttpResponse> {
        self.post_rag_chat(query, workspace_id, doc_scope, None, false)
            .await
    }

    pub async fn chat_with_format_hint(
        &self,
        query: &str,
        workspace_id: &str,
        doc_scope: &[String],
        format_hint: Option<&str>,
    ) -> anyhow::Result<HttpResponse> {
        self.post_rag_chat(query, workspace_id, doc_scope, format_hint, true)
            .await
    }

    pub async fn chat_with_format_hint_without_mock_chunk_pin(
        &self,
        query: &str,
        workspace_id: &str,
        doc_scope: &[String],
        format_hint: Option<&str>,
    ) -> anyhow::Result<HttpResponse> {
        self.post_rag_chat(query, workspace_id, doc_scope, format_hint, false)
            .await
    }

    async fn post_rag_chat(
        &self,
        query: &str,
        workspace_id: &str,
        doc_scope: &[String],
        format_hint: Option<&str>,
        pin_mock_chunk_ids: bool,
    ) -> anyhow::Result<HttpResponse> {
        set_mock_rag_codegen_query(query);
        let mut body = serde_json::json!({
            "query": query,
            "agent_type": "rag",
            "workspace_id": workspace_id,
            "doc_scope": doc_scope,
            "stream": false,
        });
        if let Some(hint) = format_hint {
            body["format_hint"] = serde_json::json!(hint);
        }
        if pin_mock_chunk_ids && !doc_scope.is_empty() {
            let mut chunk_ids = Vec::new();
            for doc_id in doc_scope {
                if let Ok(chunk_id) = self.query_first_chunk_id(doc_id).await {
                    chunk_ids.push(chunk_id);
                }
            }
            if !chunk_ids.is_empty() {
                if chunk_ids.len() == 1 {
                    set_mock_rag_codegen_chunk_id(chunk_ids.pop().unwrap());
                } else {
                    set_mock_rag_codegen_chunk_ids(chunk_ids);
                }
            }
        }
        let resp = self
            .http_client
            .post(format!("{}/api/v1/chat", self.base_url))
            .json(&body)
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_json = resp.json().await?;
        Ok(HttpResponse { status, body_json })
    }

    pub async fn chat_with_session(
        &self,
        query: &str,
        workspace_id: &str,
        doc_scope: &[String],
        session_id: &str,
    ) -> anyhow::Result<HttpResponse> {
        set_mock_rag_codegen_query(query);
        let body = serde_json::json!({
            "query": query,
            "agent_type": "rag",
            "workspace_id": workspace_id,
            "doc_scope": doc_scope,
            "session_id": session_id,
            "stream": false,
        });
        if !doc_scope.is_empty() {
            let mut chunk_ids = Vec::new();
            for doc_id in doc_scope {
                if let Ok(chunk_id) = self.query_first_chunk_id(doc_id).await {
                    chunk_ids.push(chunk_id);
                }
            }
            if !chunk_ids.is_empty() {
                if chunk_ids.len() == 1 {
                    set_mock_rag_codegen_chunk_id(chunk_ids.pop().unwrap());
                } else {
                    set_mock_rag_codegen_chunk_ids(chunk_ids);
                }
            }
        }
        let resp = self
            .http_client
            .post(format!("{}/api/v1/chat", self.base_url))
            .json(&body)
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_json = resp.json().await?;
        Ok(HttpResponse { status, body_json })
    }

    pub async fn chat_general(
        &self,
        query: &str,
        workspace_id: &str,
    ) -> anyhow::Result<HttpResponse> {
        let resp = self
            .http_client
            .post(format!("{}/api/v1/chat", self.base_url))
            .json(&serde_json::json!({
                "query": query,
                "agent_type": "chat",
                "workspace_id": workspace_id,
                "doc_scope": Vec::<String>::new(),
                "stream": false,
            }))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_json = resp.json().await?;
        Ok(HttpResponse { status, body_json })
    }

    pub async fn create_share_token(&self, workspace_id: &str) -> anyhow::Result<String> {
        let resp = self
            .http_client
            .post(format!(
                "{}/api/v1/workspaces/{workspace_id}/share",
                self.base_url
            ))
            .json(&serde_json::json!({ "role": "viewer" }))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body = resp.json::<serde_json::Value>().await?;
        if status != 200 {
            anyhow::bail!("create share failed: HTTP {status}, body: {body}");
        }
        body["share_token"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| anyhow::anyhow!("missing share_token in response: {body}"))
    }

    pub async fn chat_with_share(
        &self,
        query: &str,
        workspace_id: &str,
        share_token: &str,
    ) -> anyhow::Result<HttpResponse> {
        let resp = self
            .http_client
            .post(format!("{}/api/v1/chat", self.base_url))
            .json(&serde_json::json!({
                "query": query,
                "agent_type": "chat",
                "workspace_id": workspace_id,
                "source_type": "share",
                "source_token": share_token,
                "doc_scope": Vec::<String>::new(),
                "stream": false,
            }))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_json = resp.json().await?;
        Ok(HttpResponse { status, body_json })
    }

    pub fn embedding_call_count(&self) -> usize {
        self.embedding_call_count
            .as_ref()
            .map(|c| c.load(Ordering::SeqCst))
            .unwrap_or(0)
    }

    pub async fn search(&self, query: &str, workspace_id: &str) -> anyhow::Result<HttpResponse> {
        let resp = self
            .http_client
            .post(format!("{}/api/v1/chat", self.base_url))
            .json(&serde_json::json!({
                "query": query,
                "agent_type": "search",
                "workspace_id": workspace_id,
                "doc_scope": Vec::<String>::new(),
                "stream": false,
            }))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_json = resp.json().await?;
        Ok(HttpResponse { status, body_json })
    }

    pub async fn chat_stream_with_params(
        &self,
        params: ChatStreamParams<'_>,
        max_events: usize,
        max_wait: Duration,
    ) -> anyhow::Result<Vec<SseEvent>> {
        let mut body = serde_json::json!({
            "query": params.query,
            "agent_type": params.agent_type,
            "workspace_id": params.workspace_id,
            "doc_scope": params.doc_scope,
            "stream": true,
        });
        if let Some(session_id) = params.session_id {
            body["session_id"] = serde_json::json!(session_id);
        }
        if let Some(hint) = params.format_hint {
            body["format_hint"] = serde_json::json!(hint);
        }
        if params.debug {
            body["debug"] = serde_json::json!(true);
        }
        if params.agent_type == "rag" {
            set_mock_rag_codegen_query(params.query);
        }
        if params.pin_mock_chunk_ids && params.agent_type == "rag" && !params.doc_scope.is_empty() {
            let mut chunk_ids = Vec::new();
            for doc_id in params.doc_scope {
                if let Ok(chunk_id) = self.query_first_chunk_id(doc_id).await {
                    chunk_ids.push(chunk_id);
                }
            }
            if !chunk_ids.is_empty() {
                if chunk_ids.len() == 1 {
                    set_mock_rag_codegen_chunk_id(chunk_ids.pop().unwrap());
                } else {
                    set_mock_rag_codegen_chunk_ids(chunk_ids);
                }
            }
        }

        let resp = self
            .http_client
            .post(format!("{}/api/v1/chat", self.base_url))
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .json(&body)
            .send()
            .await?;
        let status = resp.status().as_u16();
        if status != 200 {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("chat_stream: HTTP {status}, body: {body}");
        }

        let mut resp = resp;
        let deadline = tokio::time::Instant::now() + max_wait;
        let mut parser = SseParser::new();
        let mut events: Vec<SseEvent> = Vec::new();
        loop {
            let now = tokio::time::Instant::now();
            if now >= deadline {
                anyhow::bail!(
                    "chat_stream: timed out after {max_wait:?} with {} events collected; last={:?}",
                    events.len(),
                    events.last().map(|e| e.event.clone())
                );
            }
            let remaining = deadline - now;
            let chunk = match tokio::time::timeout(remaining, resp.chunk()).await {
                Ok(Ok(Some(chunk))) => chunk,
                Ok(Ok(None)) => break,
                Ok(Err(e)) => {
                    return Err(anyhow::Error::from(e));
                }
                Err(_) => {
                    anyhow::bail!(
                        "chat_stream: timed out after {max_wait:?} with {} events collected; last={:?}",
                        events.len(),
                        events.last().map(|e| e.event.clone())
                    );
                }
            };
            for evt in parser.feed(&chunk) {
                if events.len() >= max_events {
                    anyhow::bail!(
                        "chat_stream: hit max_events={max_events} cap before stream closed (last event: {:?})",
                        evt.event
                    );
                }
                events.push(evt);
            }
        }
        Ok(events)
    }

    pub async fn chat_stream(
        &self,
        query: &str,
        workspace_id: &str,
        doc_scope: &[String],
        max_events: usize,
        max_wait: Duration,
    ) -> anyhow::Result<Vec<SseEvent>> {
        self.chat_stream_with_params(
            ChatStreamParams {
                query,
                agent_type: "rag",
                workspace_id,
                doc_scope,
                session_id: None,
                format_hint: None,
                debug: false,
                pin_mock_chunk_ids: true,
            },
            max_events,
            max_wait,
        )
        .await
    }

    /// Register a user via `/api/auth/register` and return the JWT token.
    pub async fn register_user_token(
        &self,
        email: &str,
        full_name: &str,
    ) -> anyhow::Result<String> {
        let body = serde_json::json!({
            "email": email,
            "password": "password123",
            "full_name": full_name,
            "terms_version": app_core::PUBLISHED_TERMS_VERSION,
            "privacy_version": app_core::PUBLISHED_PRIVACY_VERSION,
        });
        let resp = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?
            .post(format!("{}/api/auth/register", self.base_url))
            .json(&body)
            .send()
            .await?;
        let status = resp.status().as_u16();
        let payload = resp.json::<serde_json::Value>().await?;
        if status != 201 {
            anyhow::bail!("register failed: HTTP {status}, body: {payload}");
        }
        payload["data"]["token"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| anyhow::anyhow!("missing token in register response: {payload}"))
    }

    pub async fn create_workspace_with_token(
        &self,
        token: &str,
        name: &str,
    ) -> anyhow::Result<WorkspaceInner> {
        let resp = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()?
            .post(format!("{}/api/v1/workspaces", self.base_url))
            .header("Authorization", format!("Bearer {token}"))
            .json(&serde_json::json!({ "name": name, "description": "" }))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body = resp.json::<serde_json::Value>().await?;
        if status != 201 {
            anyhow::bail!("create notebook with token failed: HTTP {status}, body: {body}");
        }
        let wrapper: WorkspaceResponse = serde_json::from_value(body)?;
        Ok(wrapper.workspace)
    }

    pub async fn chat_with_bearer_token(
        &self,
        token: &str,
        query: &str,
        workspace_id: &str,
    ) -> anyhow::Result<HttpResponse> {
        let resp = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()?
            .post(format!("{}/api/v1/chat", self.base_url))
            .header("Authorization", format!("Bearer {token}"))
            .json(&serde_json::json!({
                "query": query,
                "agent_type": "chat",
                "workspace_id": workspace_id,
                "doc_scope": Vec::<String>::new(),
                "stream": false,
            }))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_json = resp.json().await?;
        Ok(HttpResponse { status, body_json })
    }

    pub async fn invite_notebook_member(
        &self,
        workspace_id: &str,
        email: &str,
        role: &str,
    ) -> anyhow::Result<HttpResponse> {
        let resp = self
            .http_client
            .post(format!(
                "{}/api/v1/workspaces/{workspace_id}/members/invite",
                self.base_url
            ))
            .json(&serde_json::json!({ "email": email, "role": role }))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_json = resp.json().await?;
        Ok(HttpResponse { status, body_json })
    }

    pub async fn list_workspace_members(
        &self,
        workspace_id: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let resp = self
            .http_client
            .get(format!(
                "{}/api/v1/workspaces/{workspace_id}/members",
                self.base_url
            ))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body = resp.json::<serde_json::Value>().await?;
        if status != 200 {
            anyhow::bail!("list members failed: HTTP {status}, body: {body}");
        }
        Ok(body)
    }

    pub async fn delete_document(&self, document_id: &str) -> anyhow::Result<HttpResponse> {
        let resp = self
            .http_client
            .delete(format!("{}/api/v1/documents/{document_id}", self.base_url))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_json = resp.json().await?;
        Ok(HttpResponse { status, body_json })
    }

    pub async fn create_checkout_session_with_token(
        &self,
        token: &str,
        plan_id: &str,
    ) -> anyhow::Result<HttpResponse> {
        let resp = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?
            .post(format!("{}/api/v1/billing/checkout-session", self.base_url))
            .header("Authorization", format!("Bearer {token}"))
            .json(&serde_json::json!({ "plan_id": plan_id }))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_json = resp.json().await?;
        Ok(HttpResponse { status, body_json })
    }

    pub async fn reindex_document(&self, document_id: &str) -> anyhow::Result<HttpResponse> {
        let resp = self
            .http_client
            .post(format!(
                "{}/api/v1/documents/{document_id}/reindex",
                self.base_url
            ))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_json = resp.json().await?;
        Ok(HttpResponse { status, body_json })
    }

    /// Start SSE chat, read until `start` (or first chunk), then drop the connection.
    pub async fn chat_stream_abort_capture_session(
        &self,
        query: &str,
        workspace_id: &str,
        doc_scope: &[String],
    ) -> anyhow::Result<(Vec<SseEvent>, Option<String>)> {
        let body = serde_json::json!({
            "query": query,
            "agent_type": "rag",
            "workspace_id": workspace_id,
            "doc_scope": doc_scope,
            "stream": true,
        });
        if !doc_scope.is_empty() {
            set_mock_rag_codegen_query(query);
            if let Ok(chunk_id) = self.query_first_chunk_id(&doc_scope[0]).await {
                set_mock_rag_codegen_chunk_id(chunk_id);
            }
        }
        let resp = self
            .http_client
            .post(format!("{}/api/v1/chat", self.base_url))
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .json(&body)
            .send()
            .await?;
        let status = resp.status().as_u16();
        if status != 200 {
            let body_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("chat_stream_abort: HTTP {status}, body: {body_text}");
        }
        let mut resp = resp;
        let mut parser = SseParser::new();
        let mut events = Vec::new();
        for _ in 0..4 {
            match resp.chunk().await {
                Ok(Some(chunk)) => {
                    events.extend(parser.feed(&chunk));
                    if events.iter().any(|e| e.event == "start") {
                        break;
                    }
                }
                Ok(None) => break,
                Err(_) => break,
            }
        }
        drop(resp);
        let session_id = events
            .iter()
            .find_map(|e| {
                if e.event == "start" {
                    e.data.get("session_id").and_then(|v| v.as_str())
                } else {
                    None
                }
            })
            .map(str::to_owned);
        Ok((events, session_id))
    }

    /// Start a streaming chat and drop the connection after the first SSE chunk.
    pub async fn chat_stream_abort_after_start(
        &self,
        query: &str,
        workspace_id: &str,
        doc_scope: &[String],
    ) -> anyhow::Result<Vec<SseEvent>> {
        let body = serde_json::json!({
            "query": query,
            "agent_type": "rag",
            "workspace_id": workspace_id,
            "doc_scope": doc_scope,
            "stream": true,
        });
        if !doc_scope.is_empty() {
            set_mock_rag_codegen_query(query);
            if let Ok(chunk_id) = self.query_first_chunk_id(&doc_scope[0]).await {
                set_mock_rag_codegen_chunk_id(chunk_id);
            }
        }
        let resp = self
            .http_client
            .post(format!("{}/api/v1/chat", self.base_url))
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .json(&body)
            .send()
            .await?;
        let status = resp.status().as_u16();
        if status != 200 {
            let body_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("chat_stream_abort: HTTP {status}, body: {body_text}");
        }
        let mut resp = resp;
        let mut parser = SseParser::new();
        let mut events = Vec::new();
        if let Ok(Some(chunk)) = resp.chunk().await {
            for evt in parser.feed(&chunk) {
                events.push(evt);
            }
        }
        drop(resp);
        Ok(events)
    }

    pub async fn get_workspace(&self, workspace_id: &str) -> anyhow::Result<HttpResponse> {
        let resp = self
            .http_client
            .get(format!("{}/api/v1/workspaces/{workspace_id}", self.base_url))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_json = resp.json().await?;
        Ok(HttpResponse { status, body_json })
    }

    pub async fn list_workspaces(&self) -> anyhow::Result<HttpResponse> {
        let resp = self
            .http_client
            .get(format!("{}/api/v1/workspaces", self.base_url))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_json = resp.json().await?;
        Ok(HttpResponse { status, body_json })
    }

    pub async fn update_workspace(
        &self,
        workspace_id: &str,
        name: &str,
        description: &str,
    ) -> anyhow::Result<HttpResponse> {
        let resp = self
            .http_client
            .patch(format!("{}/api/v1/workspaces/{workspace_id}", self.base_url))
            .json(&serde_json::json!({ "name": name, "description": description }))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_json = resp.json().await?;
        Ok(HttpResponse { status, body_json })
    }

    pub async fn delete_workspace(&self, workspace_id: &str) -> anyhow::Result<HttpResponse> {
        let resp = self
            .http_client
            .delete(format!("{}/api/v1/workspaces/{workspace_id}", self.base_url))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_json = resp.json().await?;
        Ok(HttpResponse { status, body_json })
    }

    pub async fn accept_notebook_invite(
        &self,
        workspace_id: &str,
        member_id: &str,
    ) -> anyhow::Result<HttpResponse> {
        let resp = self
            .http_client
            .post(format!(
                "{}/api/v1/workspaces/{workspace_id}/members/{member_id}/accept",
                self.base_url
            ))
            .json(&serde_json::json!({}))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_json = resp.json().await?;
        Ok(HttpResponse { status, body_json })
    }
}

/// Email assigned by PG `ensure_org_and_actor` for proxy-header E2E users.
pub fn local_dev_email(user_id: &str) -> String {
    format!("{user_id}@local.dev")
}
