//! Product E2E shared infrastructure.
//!
//! Design principles:
//! - HTTP black-box entry only — no direct Strategy/Runtime calls.
//! - Smoke uses real PG + local Object Store, mocks LLM/Search/Embedding via HTTP-level stubs.
//! - Protocol assertions first, then deserialize to business types.

pub mod assertions;
pub mod setup;

pub mod smoke;
pub mod integration;
pub mod failure;
pub mod tenants;
pub mod llm_real;

use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use axum::{response::IntoResponse, routing::post, Json, Router};
use uuid::Uuid;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use serde_json::json;

// ---------------------------------------------------------------------------
// Process-wide helpers
// ---------------------------------------------------------------------------

/// Ensures `cleanup_orphaned_test_containers` runs at most once per test binary.
///
/// Parallel `TestContext::new_*` calls used to trigger cleanup concurrently,
/// which could remove a Postgres container that another in-flight test was
/// actively using (since there is no shared registry of "owned" containers
/// between test processes). Running it exactly once, at the start of the
/// first context that gets created, removes that race.
async fn run_orphan_cleanup_once() {
    static DONE: OnceLock<()> = OnceLock::new();
    if DONE.get().is_none() {
        if let Err(e) = setup::cleanup_orphaned_test_containers().await {
            eprintln!("[product_e2e] orphan cleanup failed: {e}");
        }
        let _ = DONE.set(());
    }
}



/// Raw HTTP response from the test client.
///
/// All `ctx.chat()` / `ctx.upload_document()` helpers return this first.
/// Protocol-layer assertions operate on this type.
/// Product-layer assertions require deserializing `body_json` first.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub body_json: serde_json::Value,
}

impl HttpResponse {
    /// Deserialize the JSON body into a typed business response.
    pub fn into_business<T: serde::de::DeserializeOwned>(self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.body_json)
    }
}

// ---------------------------------------------------------------------------
// Server-Sent Events (SSE) parsing
// ---------------------------------------------------------------------------

/// A single SSE event parsed from a streaming chat response.
#[derive(Debug, Clone)]
pub struct SseEvent {
    /// Event name from the `event: <name>` line.
    pub event: String,
    /// JSON body from the `data: <json>` line.
    pub data: serde_json::Value,
}

/// Minimal SSE parser. Feed it raw response bytes via [`SseParser::feed`];
/// it returns any complete events it finds. Handles `event:` / `data:`
/// lines, blank-line event terminators, and `:` comment lines.
pub struct SseParser {
    buf: String,
    current_event: Option<String>,
    current_data: Option<String>,
}

impl SseParser {
    pub fn new() -> Self {
        Self {
            buf: String::new(),
            current_event: None,
            current_data: None,
        }
    }

    /// Feed a chunk of bytes; return any complete events parsed from it.
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        use std::str::from_utf8;
        let s = match from_utf8(chunk) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        self.buf.push_str(s);
        let mut out = Vec::new();
        while let Some(idx) = self.buf.find('\n') {
            let line: String = self.buf.drain(..=idx).collect();
            let line = line.trim_end_matches(&['\r', '\n'][..]).to_string();
            if line.is_empty() {
                // Event terminator: emit if we have data
                if let (Some(event), Some(data)) = (self.current_event.take(), self.current_data.take()) {
                    let parsed = serde_json::from_str(&data)
                        .unwrap_or(serde_json::Value::String(data));
                    out.push(SseEvent { event, data: parsed });
                }
            } else if let Some(rest) = line.strip_prefix("event:") {
                self.current_event = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("data:") {
                // Spec: concatenate multiple data: lines with \n
                let piece = rest.trim_start();
                match &mut self.current_data {
                    Some(d) => {
                        d.push('\n');
                        d.push_str(piece);
                    }
                    None => {
                        self.current_data = Some(piece.to_string());
                    }
                }
            } else if line.starts_with(':') {
                // Comment / keep-alive; ignore
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Business response types (re-exported from production code)
// ---------------------------------------------------------------------------

pub use common::{ChatResponse, Citation, DegradeTraceItem, DocumentStatus};

// ---------------------------------------------------------------------------
// Upload response (document upload)
// ---------------------------------------------------------------------------

/// Response from `POST /api/v1/notebooks/{id}/documents`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct UploadResponse {
    pub document_id: String,
    pub notebook_id: String,
    pub upload_url: String,
    #[serde(default)]
    pub status: u16,
}

/// Notebook creation response wrapper.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct NotebookResponse {
    pub notebook: NotebookInner,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct NotebookInner {
    pub id: String,
    pub title: String,
}

// ---------------------------------------------------------------------------
// TestContext
// ---------------------------------------------------------------------------

/// Per-test execution context.
///
/// Created via `TestContext::new_smoke().await` or `new_smoke_with_rag().await`.
/// Automatically cleans up on drop (containers, temp dirs, worker process, HTTP server, mock servers).
pub struct TestContext {
    pub http_client: reqwest::Client,
    pub base_url: String,
    pg_container_name: String,
    milvus_container_name: Option<String>,
    /// True when Milvus is an external (non-test-owned) instance and must NOT be stopped in `Drop`.
    milvus_is_external: bool,
    worker: Option<tokio::process::Child>,
    server_abort: Option<tokio::sync::oneshot::Sender<()>>,
    #[allow(dead_code)]
    object_store_dir: tempfile::TempDir,
    pg_url: String,
    mock_llm_abort: Option<tokio::sync::oneshot::Sender<()>>,
    mock_embedding_abort: Option<tokio::sync::oneshot::Sender<()>>,
    mock_search_abort: Option<tokio::sync::oneshot::Sender<()>>,
    search_should_429: Option<Arc<AtomicBool>>,
    embedding_should_503: Option<Arc<AtomicBool>>,
    worker_log_path: Option<std::path::PathBuf>,
}

/// Default test org/user IDs.
///
/// Kept as public constants for tests that need a stable, well-known
/// identity (e.g. checking that the production auth path correctly
/// handles a specific UUID format). New tests that do not need a fixed
/// identity should use [`unique_test_identity`] instead, so that
/// parallel tests do not share the same rate-limit bucket.
pub const DEFAULT_TEST_ORG_ID: &str = "00000000-0000-0000-0000-000000000001";
pub const DEFAULT_TEST_USER_ID: &str = "00000000-0000-0000-0000-000000000001";

/// Generate a unique `(org_id, user_id)` pair for a test context so that
/// each test gets its own rate-limit bucket and does not collide with
/// other tests running in parallel.
pub fn unique_test_identity() -> (String, String) {
    use uuid::Uuid;
    (
        Uuid::new_v4().to_string(),
        Uuid::new_v4().to_string(),
    )
}

/// Auth headers for a specific org/user (used by `build_smoke` and by
/// multi-tenant isolation tests).
fn test_auth_headers_for(org_id: &str, user_id: &str) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("x-org-id", org_id.parse().unwrap());
    headers.insert("x-user-id", user_id.parse().unwrap());
    headers.insert("x-permissions", "external_network".parse().unwrap());
    headers
}

impl TestContext {
    /// Create a Smoke E2E context (no RAG).
    pub async fn new_smoke() -> Self {
        Self::build_smoke(false, 300, None, false).await
    }

    /// Create a Smoke E2E context with RAG enabled (Milvus + mock embedding/LLM).
    pub async fn new_smoke_with_rag() -> Self {
        Self::build_smoke(true, 300, None, false).await
    }

    /// Create a Smoke E2E context with RAG and a custom worker per-task timeout.
    pub async fn new_smoke_with_rag_and_timeout(worker_timeout_secs: u64) -> Self {
        Self::build_smoke(true, worker_timeout_secs, None, false).await
    }

    /// Create a Smoke E2E context with RAG and a specific org/user identity.
    ///
    /// Used by multi-tenant isolation tests to construct a second client
    /// in a different org and verify that one org cannot read another org's data.
    pub async fn new_smoke_with_rag_and_org(org_id: &str, user_id: &str) -> Self {
        let identity = Some((org_id.to_string(), user_id.to_string()));
        Self::build_smoke(true, 300, identity, false).await
    }

    async fn build_smoke(
        enable_rag: bool,
        worker_timeout_secs: u64,
        identity: Option<(String, String)>,
        use_real_llm: bool,
    ) -> Self {
        // 0. Best-effort: clean up orphan containers from previous test runs.
        //    Run at most once per process to avoid races where parallel tests
        //    remove each other's still-in-use containers.
        //    Failures are non-fatal — log and continue.
        run_orphan_cleanup_once().await;

        // Each context gets a unique (org, user) pair by default so that
        // parallel tests do not share the same rate-limit bucket and
        // trigger 429s. Tests that want to share a bucket (e.g. the
        // cross-org isolation test) pass an explicit `identity`.
        let identity = identity.or_else(|| Some(unique_test_identity()));

        // 1. Start Postgres
        let pg_url = setup::start_postgres().await.expect("start postgres");
        let pg_container_name = format!("avrag-test-pg-{}", pg_url.rsplit(':').next().unwrap());

        // 2. Start Milvus if RAG enabled
        let (milvus_url, milvus_container_name, milvus_is_external) = if enable_rag {
            let inst = setup::start_milvus().await.expect("start milvus");
            let name = inst.container_name.clone();
            (Some(inst.url), name, inst.is_external)
        } else {
            (None, None, false)
        };

        // 3. Temp object store
        let object_store_dir = setup::create_temp_object_store();
        let object_root = object_store_dir.path().to_string_lossy().to_string();

        // 4. Start mock LLM unless we're using a real LLM.
        let (mock_llm_url, mock_llm_abort) = if use_real_llm {
            (String::new(), None)
        } else {
            let (url, abort) = start_mock_llm_server().await;
            (url, Some(abort))
        };

        // 5. Start mock Search (always — needed by Search tests)
        let (mock_search_url, mock_search_abort, search_should_429) = start_mock_search_server().await;

        // 6. Start mock Embedding if RAG enabled and not using real LLM.
        let (mock_embedding_url, mock_embedding_abort, embedding_should_503) = if enable_rag && !use_real_llm {
            let (url, abort, flag) = start_mock_embedding_server().await;
            (Some(url), Some(abort), Some(flag))
        } else {
            (None, None, None)
        };

        // 5. Set env vars for AppConfig
        unsafe {
            std::env::set_var("DATABASE_URL", &pg_url);
            std::env::set_var("AVRAG_RUN_MIGRATIONS", "true");
            std::env::set_var("AVRAG_OBJECT_ROOT", &object_root);
            std::env::set_var("AVRAG_ENABLE_RAG", if enable_rag { "true" } else { "false" });
            std::env::set_var("REDIS_URL", ""); // disable Redis
            std::env::set_var("AVRAG_PUBLIC_BASE_URL", "http://127.0.0.1:8080");

            if let Some(ref url) = milvus_url {
                std::env::set_var("MILVUS_URL", url);
                std::env::set_var("MILVUS_TOKEN", "");
                std::env::set_var("MILVUS_DATABASE", "default");
                // Fixed prefix for reproducible debugging
                let prefix = "avrag_e2e_test".to_string();
                std::env::set_var("MILVUS_COLLECTION_PREFIX", &prefix);
            }
            // LLM config: keep real values when use_real_llm is set; otherwise inject mocks.
            if !use_real_llm {
                std::env::set_var("AGENT_LLM_BASE_URL", &mock_llm_url);
                std::env::set_var("AGENT_LLM_API_KEY", "mock");
                std::env::set_var("AGENT_LLM_MODEL", "mock-llm");
                std::env::set_var("MEMORY_LLM_BASE_URL", &mock_llm_url);
                std::env::set_var("MEMORY_LLM_API_KEY", "mock");
                std::env::set_var("MEMORY_LLM_MODEL", "mock-llm");
                std::env::set_var("INGESTION_LLM_BASE_URL", &mock_llm_url);
                std::env::set_var("INGESTION_LLM_API_KEY", "mock");
                std::env::set_var("INGESTION_LLM_MODEL", "mock-llm");
            }

            // Search config (always)
            std::env::set_var("SEARCH_PROVIDER", "brave_llm_context");
            std::env::set_var("SEARCH_BASE_URL", &mock_search_url);
            std::env::set_var("SEARCH_API_KEY", "mock");

            if let Some(ref url) = mock_embedding_url {
                std::env::set_var("EMBEDDING_BASE_URL", url);
                std::env::set_var("EMBEDDING_API_KEY", "mock");
                std::env::set_var("EMBEDDING_MODEL", "mock-embedding");
            }
            // When use_real_llm is true, EMBEDDING_* is already set from .env above.
        }

        // 6. Bootstrap AppState and start HTTP server
        let config = app::AppConfig::from_env();
        let state = app::AppState::bootstrap(config.clone()).await.expect("bootstrap AppState");

        let router = transport_http::build_router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base_url = format!("http://{}", listener.local_addr().unwrap());

        let (abort_tx, abort_rx) = tokio::sync::oneshot::channel::<()>();
        tokio::spawn(async move {
            let server = axum::serve(listener, router);
            tokio::select! {
                _ = server => {},
                _ = abort_rx => {},
            }
        });

        // 7. Start worker process
        let worker_binary = setup::find_worker_binary().await.expect("find worker binary");
        let worker_log_path = object_store_dir.path().join("worker.log");
        let mut cmd = tokio::process::Command::new(&worker_binary);
        cmd.env("DATABASE_URL", &pg_url)
            .env("AVRAG_RUN_MIGRATIONS", "false")
            .env("AVRAG_OBJECT_ROOT", &object_root)
            .env("AVRAG_ENABLE_RAG", if enable_rag { "true" } else { "false" })
            .env("REDIS_URL", "")
            .env("AVRAG_PUBLIC_BASE_URL", &base_url)
            .env("AVRAG_WORKER_ID", "test-worker")
            .env("AVRAG_WORKER_POLL_SECS", "1")
            .env("AVRAG_INGESTION_TASK_TIMEOUT_SECS", worker_timeout_secs.to_string())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

        if let Some(ref url) = milvus_url {
            cmd.env("MILVUS_URL", url)
               .env("MILVUS_TOKEN", "")
               .env("MILVUS_DATABASE", "default")
               .env("MILVUS_COLLECTION_PREFIX", std::env::var("MILVUS_COLLECTION_PREFIX").unwrap_or_default());
        }
        // Worker LLM env: real or mock depending on mode.
        if use_real_llm {
            for key in ["AGENT_LLM_BASE_URL", "AGENT_LLM_API_KEY", "AGENT_LLM_MODEL",
                        "MEMORY_LLM_BASE_URL", "MEMORY_LLM_API_KEY", "MEMORY_LLM_MODEL",
                        "INGESTION_LLM_BASE_URL", "INGESTION_LLM_API_KEY", "INGESTION_LLM_MODEL"] {
                if let Ok(v) = std::env::var(key) {
                    cmd.env(key, v);
                }
            }
            for key in ["EMBEDDING_BASE_URL", "EMBEDDING_API_KEY", "EMBEDDING_MODEL"] {
                if let Ok(v) = std::env::var(key) {
                    cmd.env(key, v);
                }
            }
        } else {
            cmd.env("AGENT_LLM_BASE_URL", &mock_llm_url)
               .env("AGENT_LLM_API_KEY", "mock")
               .env("AGENT_LLM_MODEL", "mock-llm")
               .env("MEMORY_LLM_BASE_URL", &mock_llm_url)
               .env("MEMORY_LLM_API_KEY", "mock")
               .env("MEMORY_LLM_MODEL", "mock-llm")
               .env("INGESTION_LLM_BASE_URL", &mock_llm_url)
               .env("INGESTION_LLM_API_KEY", "mock")
               .env("INGESTION_LLM_MODEL", "mock-llm");

            if let Some(ref url) = mock_embedding_url {
                cmd.env("EMBEDDING_BASE_URL", url)
                   .env("EMBEDDING_API_KEY", "mock")
                   .env("EMBEDDING_MODEL", "mock-embedding");
            }
        }

        // Worker always gets Search env vars (mock for smoke tests).
        cmd.env("SEARCH_PROVIDER", "brave_llm_context")
           .env("SEARCH_BASE_URL", &mock_search_url)
           .env("SEARCH_API_KEY", "mock");

        let mut worker = cmd.spawn().expect("spawn worker");

        // Drain worker stdout/stderr into a log file for failure artifact capture.
        let log_path = worker_log_path.clone();
        if let Some(stdout) = worker.stdout.take() {
            let mut reader = tokio::io::BufReader::new(stdout).lines();
            tokio::spawn(async move {
                let mut file = match tokio::fs::File::create(&log_path).await {
                    Ok(f) => f,
                    Err(_) => return,
                };
                while let Ok(Some(line)) = reader.next_line().await {
                    let _ = file.write_all(line.as_bytes()).await;
                    let _ = file.write_all(b"\n").await;
                }
            });
        }
        if let Some(stderr) = worker.stderr.take() {
            let log_path = worker_log_path.clone();
            let mut reader = tokio::io::BufReader::new(stderr).lines();
            tokio::spawn(async move {
                let mut file = match tokio::fs::OpenOptions::new().append(true).create(true).open(&log_path).await {
                    Ok(f) => f,
                    Err(_) => return,
                };
                while let Ok(Some(line)) = reader.next_line().await {
                    let _ = file.write_all(line.as_bytes()).await;
                    let _ = file.write_all(b"\n").await;
                }
            });
        }

        // Give worker a moment to start
        tokio::time::sleep(Duration::from_secs(1)).await;

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .default_headers(match &identity {
                Some((org, user)) => test_auth_headers_for(org, user),
                None => unreachable!("identity is always Some after .or_else above"),
            })
            .build()
            .expect("reqwest client build");

        Self {
            http_client: client,
            base_url,
            pg_container_name,
            milvus_container_name,
            milvus_is_external,
            worker: Some(worker),
            server_abort: Some(abort_tx),
            object_store_dir,
            pg_url,
            mock_llm_abort,
            mock_embedding_abort,
            mock_search_abort: Some(mock_search_abort),
            search_should_429: Some(search_should_429),
            embedding_should_503,
            worker_log_path: Some(worker_log_path),
        }
    }

    // -----------------------------------------------------------------------
    // HTTP helpers
    // -----------------------------------------------------------------------

    /// Create a notebook and return its inner data.
    pub async fn create_notebook(&self, name: &str) -> anyhow::Result<NotebookInner> {
        let resp = self
            .http_client
            .post(format!("{}/api/v1/notebooks", self.base_url))
            .json(&serde_json::json!({ "name": name, "description": "" }))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body = resp.json::<serde_json::Value>().await?;
        if status != 201 {
            anyhow::bail!("create notebook failed: HTTP {status}, body: {body}");
        }
        let wrapper: NotebookResponse = serde_json::from_value(body)?;
        Ok(wrapper.notebook)
    }

    /// Upload a fixture file and return the document ID.
    pub async fn upload_document(&self, fixture: &str) -> anyhow::Result<UploadResponse> {
        let notebook = self.create_notebook("test-notebook").await?;
        self.upload_document_to_notebook(fixture, &notebook.id).await
    }

    /// Upload a fixture file to an existing notebook.
    pub async fn upload_document_to_notebook(
        &self,
        fixture: &str,
        notebook_id: &str,
    ) -> anyhow::Result<UploadResponse> {
        let content = setup::load_fixture(fixture)?;
        let bytes = content.into_bytes();

        let resp = self
            .http_client
            .post(format!(
                "{}/api/v1/notebooks/{}/documents",
                self.base_url, notebook_id
            ))
            .json(&serde_json::json!({
                "filename": fixture,
                "file_size": bytes.len(),
                "mime_type": "text/plain",
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

        // PUT the file bytes
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

        Ok(UploadResponse {
            document_id,
            notebook_id: notebook_id.to_string(),
            upload_url: String::new(),
            status,
        })
    }

    /// Poll ingestion status until completed, failed, or timeout.
    ///
    /// On transient errors (network blip, 5xx from server, JSON parse failure)
    /// the call retries up to 3 times with 200ms backoff before propagating.
    /// On `4xx` (e.g. document not found) the error is returned immediately.
    /// If the worker process exits during polling, the call fails fast
    /// instead of waiting for the full timeout.
    ///
    /// Takes `&mut self` so it can call `Child::try_wait` on the worker
    /// handle to detect early worker death. Callers should not hold any
    /// other reference to `ctx` across the `.await`.
    pub async fn wait_for_ingestion(
        &mut self,
        doc_id: &str,
        timeout: Duration,
    ) -> anyhow::Result<DocumentStatus> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut last_status = String::new();
        loop {
            // Fail fast if the worker died (avoids hanging until timeout).
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
                "failed" => return Ok(DocumentStatus::Failed),
                _ => {}
            }
            if tokio::time::Instant::now() > deadline {
                anyhow::bail!("wait_for_ingestion timed out after {timeout:?}, last status={last_status}");
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// GET the document status with up to 3 retries on transient errors.
    /// Returns the parsed JSON body on success.
    async fn fetch_status_with_retry(
        &self,
        doc_id: &str,
    ) -> anyhow::Result<serde_json::Value> {
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

    /// Send a RAG chat query and return the raw HTTP response.
    pub async fn chat(
        &self,
        query: &str,
        notebook_id: &str,
        doc_scope: &[String],
    ) -> anyhow::Result<HttpResponse> {
        self.chat_with_format_hint(query, notebook_id, doc_scope, None).await
    }

    /// Send a RAG chat query with an optional format_hint.
    pub async fn chat_with_format_hint(
        &self,
        query: &str,
        notebook_id: &str,
        doc_scope: &[String],
        format_hint: Option<&str>,
    ) -> anyhow::Result<HttpResponse> {
        let mut body = serde_json::json!({
            "query": query,
            "agent_type": "rag",
            "notebook_id": notebook_id,
            "doc_scope": doc_scope,
            "stream": false,
        });
        if let Some(hint) = format_hint {
            body["format_hint"] = serde_json::json!(hint);
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

    /// Send a Search query and return the raw HTTP response.
    pub async fn search(
        &self,
        query: &str,
        notebook_id: &str,
    ) -> anyhow::Result<HttpResponse> {
        let resp = self
            .http_client
            .post(format!("{}/api/v1/chat", self.base_url))
            .json(&serde_json::json!({
                "query": query,
                "agent_type": "search",
                "notebook_id": notebook_id,
                "doc_scope": Vec::<String>::new(),
                "stream": false,
            }))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_json = resp.json().await?;
        Ok(HttpResponse { status, body_json })
    }

    /// Send a streaming RAG chat request and return the raw SSE event stream.
    ///
    /// Reads the stream until the response body is fully consumed
    /// (production closes the HTTP response after the `done` / `error`
    /// event). As a safety net, the function bails after `max_wait`.
    /// `max_events` is no longer a stop condition — it is only used
    /// to bail if the stream is genuinely unbounded (e.g. the
    /// production bug where `done` is never emitted).
    pub async fn chat_stream(
        &self,
        query: &str,
        notebook_id: &str,
        doc_scope: &[String],
        max_events: usize,
        max_wait: Duration,
    ) -> anyhow::Result<Vec<SseEvent>> {
        let resp = self
            .http_client
            .post(format!("{}/api/v1/chat", self.base_url))
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .json(&serde_json::json!({
                "query": query,
                "agent_type": "rag",
                "notebook_id": notebook_id,
                "doc_scope": doc_scope,
                "stream": true,
            }))
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
                Ok(Ok(None)) => break, // stream closed — this is the normal exit
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

    // -----------------------------------------------------------------------
    // Failure artifact capture
    // -----------------------------------------------------------------------

    /// Override the ingestion task max_attempts for a document.
    ///
    /// Useful in failure-scenario tests where we want a parser error to
    /// dead-letter immediately instead of waiting through the full retry
    /// backoff chain (≈ 7.5 min with default max_attempts=5).
    pub async fn set_ingestion_max_attempts(
        &self,
        document_id: &str,
        max_attempts: i32,
    ) -> anyhow::Result<()> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let doc_id = Uuid::parse_str(document_id)?;
        sqlx::query(
            r#"
            update ingestion_tasks
            set max_attempts = $1,
                updated_at = now()
            where document_id = $2
            "#,
        )
        .bind(max_attempts.max(1))
        .bind(doc_id)
        .execute(&pool)
        .await?;
        Ok(())
    }

    /// Toggle mock search server to return 429 (rate limit).
    pub fn set_search_429(&self, value: bool) {
        if let Some(ref flag) = self.search_should_429 {
            flag.store(value, Ordering::SeqCst);
        }
    }

    /// Toggle mock embedding server to return 503 (service unavailable).
    pub fn set_embedding_503(&self, value: bool) {
        if let Some(ref flag) = self.embedding_should_503 {
            flag.store(value, Ordering::SeqCst);
        }
    }

    /// Save debugging artifacts on test failure.
    pub fn save_failure_artifacts(
        &self,
        test_name: &str,
        response_json: Option<&serde_json::Value>,
    ) {
        let now = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let short_commit = option_env!("GITHUB_SHA")
            .map(|s| &s[..s.len().min(8)])
            .unwrap_or("local");
        let run_id = format!("e2e_{now}_{short_commit}");
        let out_dir = std::path::PathBuf::from(
            env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("e2e_output")
            .join(&run_id)
            .join(test_name);

        let _ = std::fs::create_dir_all(&out_dir);

        if let Some(body) = response_json {
            let _ = std::fs::write(out_dir.join("response_body.json"), serde_json::to_string_pretty(body).unwrap_or_default());
        }

        if let Some(ref log_path) = self.worker_log_path {
            if log_path.exists() {
                let _ = std::fs::copy(log_path, out_dir.join("worker_logs.txt"));
            }
        }
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        // Stop worker — fire-and-forget SIGKILL, don't wait
        if let Some(mut worker) = self.worker.take() {
            let _ = worker.start_kill();
        }
        // Stop HTTP server
        if let Some(tx) = self.server_abort.take() {
            let _ = tx.send(());
        }
        // Stop mock servers
        if let Some(tx) = self.mock_llm_abort.take() {
            let _ = tx.send(());
        }
        if let Some(tx) = self.mock_embedding_abort.take() {
            let _ = tx.send(());
        }
        if let Some(tx) = self.mock_search_abort.take() {
            let _ = tx.send(());
        }
        // Stop Postgres container — fire-and-forget
        let container = self.pg_container_name.clone();
        let _ = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                setup::stop_postgres(&container).await;
            });
        });
        // Stop Milvus container — fire-and-forget, but only if we started it.
        if !self.milvus_is_external
            && let Some(ref container) = self.milvus_container_name
        {
            let c = container.clone();
            let _ = std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    setup::stop_milvus(&c).await;
                });
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Mock servers
// ---------------------------------------------------------------------------

/// Names of the canned LLM responses served by [`mock_llm_handler`].
///
/// Tests can pin a call to a specific route by sending the
/// `X-Mock-Route` request header (the production LLM client never
/// sets this header, so production calls always fall through to
/// system-prompt matching).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockLlmRoute {
    RagPlanner,
    RagCoverageEvaluator,
    RagAnswer,
    SearchPlanner,
    SearchCoverageEvaluator,
    SearchAnswer,
    FormatSkillPpt,
    FormatSkillHtml,
    Fallback,
}

impl MockLlmRoute {
    /// Return the canned response body for this route.
    fn canned_response(self) -> &'static str {
        match self {
            Self::RagPlanner => r#"{"calls": [{"tool": "dense_retrieval", "version": "1.0", "args": {"queries": ["antifragility Taleb summary"], "modality": "text", "top_k": 10}}], "next_step": "answer"}"#,
            Self::RagCoverageEvaluator => r#"{"decision": "sufficient", "dimensions": [{"name": "coverage", "attempted": true, "covered": true, "retrieved_count": 3, "query_ids": ["q1"], "status": "covered_strong"}], "next_actions": [], "reasoning": "good"}"#,
            Self::RagAnswer => "Based on the document, antifragility is a property of systems that increase in capability, resilience, or robustness as a result of stressors, shocks, volatility, noise, mistakes, faults, attacks, or failures. The concept was developed by Nassim Nicholas Taleb.",
            Self::SearchPlanner => r#"{"sub_queries": ["Tokyo weather today"], "intent_summary": "The user wants to know the current weather in Tokyo.", "needs_clarification": false}"#,
            Self::SearchCoverageEvaluator => r#"{"decision": "sufficient", "dimensions": [{"name": "coverage", "attempted": true, "covered": true, "retrieved_count": 1, "query_ids": ["q1"], "status": "covered_strong"}], "next_actions": [], "reasoning": "good"}"#,
            Self::SearchAnswer => "The weather in Tokyo today is sunny with a high of 25°C [[1]].",
            Self::FormatSkillPpt => "<html><body><div class=\"slide\"><h1>Slide 1</h1><p>Summary of antifragility</p></div><div class=\"slide\"><h1>Slide 2</h1><p>Key concepts</p></div></body></html>",
            Self::FormatSkillHtml => "<html><body><h1>Antifragility</h1><p>Antifragility is a property of systems that benefit from stress.</p></body></html>",
            Self::Fallback => "This document discusses antifragility, a concept by Nassim Nicholas Taleb describing systems that benefit from shock and disorder.",
        }
    }

    /// Resolve a route from the optional `X-Mock-Route` header value.
    /// Returns `None` if the header is missing or has an unknown value.
    fn from_header(value: &str) -> Option<Self> {
        match value.trim() {
            "rag-planner" => Some(Self::RagPlanner),
            "rag-eval" => Some(Self::RagCoverageEvaluator),
            "rag-answer" => Some(Self::RagAnswer),
            "search-planner" => Some(Self::SearchPlanner),
            "search-eval" => Some(Self::SearchCoverageEvaluator),
            "search-answer" => Some(Self::SearchAnswer),
            "format-ppt" => Some(Self::FormatSkillPpt),
            "format-html" => Some(Self::FormatSkillHtml),
            "fallback" => Some(Self::Fallback),
            _ => None,
        }
    }

    /// Resolve a route by inspecting the system prompt text. This is the
    /// fallback path used by the production LLM client (which does NOT
    /// set `X-Mock-Route`).
    ///
    /// ## Order matters
    ///
    /// The format-skill catalog (`- ppt-generation (v1.0): ...`,
    /// `- html-renderer (v1.0): ...`) is appended to **every** RAG
    /// answer-phase system prompt, so the format-skill checks must
    /// come BEFORE the generic RAG answer check. Same logic for the
    /// search answer: the user prompt template always includes a
    /// `Search results:` line, so the search-answer check must be
    /// early enough to not be masked by later fallbacks.
    fn from_system_prompt(system_prompt: &str, user_prompt: &str) -> Self {
        // 1. RAG planner
        if system_prompt.contains("Context OS RAG retrieval planner") {
            Self::RagPlanner
        // 2. RAG coverage evaluator
        } else if system_prompt.contains("Context OS retrieval coverage evaluator") {
            Self::RagCoverageEvaluator
        // 3. Format skills (must come before RAG answer — the catalog
        //    line is appended to every RAG answer prompt).
        } else if system_prompt.contains("ppt-generation") {
            Self::FormatSkillPpt
        } else if system_prompt.contains("html-renderer") {
            Self::FormatSkillHtml
        // 4. RAG answer (default for the answer phase)
        } else if system_prompt.contains("Context OS RAG answer agent") {
            Self::RagAnswer
        // 5. Search pipeline
        } else if system_prompt.contains("Context OS Web Search planner") {
            Self::SearchPlanner
        } else if system_prompt.contains("Context OS web-search coverage evaluator") {
            Self::SearchCoverageEvaluator
        } else if system_prompt.contains("Answer the user's original web-search question")
            || user_prompt.contains("Search results:")
        {
            Self::SearchAnswer
        // 6. Fallback (e.g. summary generation)
        } else {
            Self::Fallback
        }
    }
}
async fn start_mock_llm_server() -> (String, tokio::sync::oneshot::Sender<()>) {
    let app = Router::new()
        .route(
            "/chat/completions",
            post(mock_llm_handler).layer(axum::extract::DefaultBodyLimit::max(8 * 1024 * 1024)),
        );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind mock llm");
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{port}");

    let (abort_tx, abort_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let server = axum::serve(listener, app);
        tokio::select! {
            _ = server => {},
            _ = abort_rx => {},
        }
    });

    (base_url, abort_tx)
}

/// Start a mock Embedding HTTP server on an ephemeral port.
///
/// Returns (base_url, abort_sender, embedding_should_503_flag).
async fn start_mock_embedding_server() -> (String, tokio::sync::oneshot::Sender<()>, Arc<AtomicBool>) {
    let embedding_should_503 = Arc::new(AtomicBool::new(false));
    let flag = embedding_should_503.clone();

    let app = Router::new()
        .route("/embeddings", post(move |req| mock_embedding_handler(req, flag.clone())));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind mock embedding");
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{port}");

    let (abort_tx, abort_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let server = axum::serve(listener, app);
        tokio::select! {
            _ = server => {},
            _ = abort_rx => {},
        }
    });

    (base_url, abort_tx, embedding_should_503)
}

async fn mock_llm_handler(
    headers: axum::http::HeaderMap,
    Json(req): Json<serde_json::Value>,
) -> axum::response::Response {
    let messages = req["messages"].as_array().cloned().unwrap_or_default();
    let system_prompt = messages
        .first()
        .and_then(|m| m["content"].as_str())
        .unwrap_or("");
    let user_prompt = messages
        .get(1)
        .and_then(|m| m["content"].as_str())
        .unwrap_or("");

    // 1. Header-based routing (explicit, takes priority).
    let route = headers
        .get("x-mock-route")
        .and_then(|v| v.to_str().ok())
        .and_then(MockLlmRoute::from_header)
        .unwrap_or_else(|| MockLlmRoute::from_system_prompt(system_prompt, user_prompt));

    let content = route.canned_response();
    let is_stream = req
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if is_stream {
        // SSE format expected by ChatCompletionStreamParser.
        // Emit 1-char deltas (token-by-token) so `MessageDelta`
        // events fire frequently and the production `complete_stream`
        // path is exercised end-to-end.
        let mut body = String::new();
        for (i, ch) in content.chars().enumerate() {
            let delta_json = json!({
                "choices": [{
                    "delta": {"content": ch.to_string()},
                    "index": 0
                }],
                "model": "mock-llm"
            });
            body.push_str(&format!("data: {delta_json}\n\n"));
            // Small inter-chunk gap so the client sees multiple chunks
            // (production has variable latency between tokens).
            if i % 8 == 7 {
                body.push_str(": keep-alive\n\n");
            }
        }
        // Final chunk with usage so the parser records it.
        let final_json = json!({
            "choices": [{"delta": {}, "index": 0, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 100, "completion_tokens": content.len(), "total_tokens": 100 + content.len()},
            "model": "mock-llm"
        });
        body.push_str(&format!("data: {final_json}\n\n"));
        body.push_str("data: [DONE]\n\n");

        axum::response::Response::builder()
            .status(200)
            .header("content-type", "text/event-stream")
            .header("cache-control", "no-cache")
            .body(axum::body::Body::from(body))
            .unwrap()
    } else {
        axum::Json(json!({
            "choices": [{"message": {"role": "assistant", "content": content}}],
            "usage": {"prompt_tokens": 100, "completion_tokens": content.len(), "total_tokens": 100 + content.len()},
            "model": "mock-llm"
        }))
        .into_response()
    }
}

#[cfg(test)]
mod mock_routing_tests {
    use super::MockLlmRoute;

    #[test]
    fn header_routing_recognizes_all_known_routes() {
        for value in [
            "rag-planner",
            "rag-eval",
            "rag-answer",
            "search-planner",
            "search-eval",
            "search-answer",
            "format-ppt",
            "format-html",
            "fallback",
        ] {
            assert!(
                MockLlmRoute::from_header(value).is_some(),
                "header value '{value}' should map to a route"
            );
        }
    }

    #[test]
    fn header_routing_returns_none_for_unknown_value() {
        assert_eq!(MockLlmRoute::from_header(""), None);
        assert_eq!(MockLlmRoute::from_header("garbage"), None);
        assert_eq!(MockLlmRoute::from_header("RAG-PLANNER"), None); // case-sensitive
    }

    #[test]
    fn system_prompt_routing_orders_format_skills_before_rag_answer() {
        // The RAG answer phase appends the format-skill catalog to the
        // system prompt, so the system prompt contains BOTH the RAG
        // answer marker AND the format-skill IDs. The format skill
        // must win; if we ever re-order and put RAG answer first, the
        // format_output integration tests will start failing with
        // 'expected slide in formatted answer'.
        let prompt = "\
You are the Context OS RAG answer agent.

## Available Output Formats

- ppt-generation (v1.0): Load when the user requests a slide deck
- html-renderer (v1.0): Load when the user asks for HTML

## Selected Format Skills

You are the Context OS presentation generation assistant.
When the user asks for a presentation, output structured JSON.
";
        let route = MockLlmRoute::from_system_prompt(prompt, "");
        assert_eq!(
            route,
            MockLlmRoute::FormatSkillPpt,
            "format-skill catalog in RAG answer prompt must route to PPT, not generic RAG answer"
        );
    }

    #[test]
    fn system_prompt_routing_picks_rag_planner_for_planner_marker() {
        let prompt = "You are the Context OS RAG retrieval planner. Given a query, decompose it into tool calls.";
        assert_eq!(
            MockLlmRoute::from_system_prompt(prompt, ""),
            MockLlmRoute::RagPlanner
        );
    }

    #[test]
    fn system_prompt_routing_falls_back_when_no_marker_matches() {
        let prompt = "You are a generic helpful assistant.";
        let user = "Hello";
        assert_eq!(
            MockLlmRoute::from_system_prompt(prompt, user),
            MockLlmRoute::Fallback
        );
    }
}

async fn mock_embedding_handler(
    Json(req): Json<serde_json::Value>,
    embedding_should_503: Arc<AtomicBool>,
) -> axum::response::Response {
    if embedding_should_503.load(Ordering::SeqCst) {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "embedding service unavailable" })),
        )
            .into_response();
    }

    let texts = req["input"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();

    let dim = req["dimensions"].as_u64().unwrap_or(1024) as usize;
    // All vectors identical so dense retrieval always returns high similarity.
    let vec: Vec<f32> = (0..dim).map(|j| 0.1_f32 + (j % 10) as f32 * 0.01).collect();
    let data: Vec<serde_json::Value> = texts.iter().map(|_| json!({"embedding": vec})).collect();

    Json(json!({ "data": data, "model": "mock-embedding" })).into_response()
}

/// Start a mock Brave Search HTTP server on an ephemeral port.
///
/// Returns (base_url, abort_sender, search_should_429_flag).
async fn start_mock_search_server() -> (String, tokio::sync::oneshot::Sender<()>, Arc<AtomicBool>) {
    let search_should_429 = Arc::new(AtomicBool::new(false));
    let flag = search_should_429.clone();

    let flag2 = flag.clone();
    let app = Router::new()
        .route("/res/v1/llm/context", post(move |req| mock_search_handler(req, flag.clone())))
        .route("/res/v1/news/search", post(move |req| mock_search_handler(req, flag2.clone())));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind mock search");
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{port}");

    let (abort_tx, abort_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let server = axum::serve(listener, app);
        tokio::select! {
            _ = server => {},
            _ = abort_rx => {},
        }
    });

    (base_url, abort_tx, search_should_429)
}

async fn mock_search_handler(
    Json(req): Json<serde_json::Value>,
    search_should_429: Arc<AtomicBool>,
) -> axum::response::Response {
    if search_should_429.load(Ordering::SeqCst) {
        return (
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            Json(json!({ "error": "rate limit exceeded" })),
        )
            .into_response();
    }

    let _query = req["q"].as_str().unwrap_or("unknown");
    Json(json!({
        "grounding": {
            "generic": [
                {
                    "url": "https://example.com/weather-tokyo",
                    "title": "Tokyo Weather Today",
                    "snippets": ["Sunny with a high of 25°C in Tokyo today."]
                }
            ],
            "map": []
        },
        "sources": {
            "https://example.com/weather-tokyo": {
                "title": "Tokyo Weather Today",
                "hostname": "example.com"
            }
        }
    }))
    .into_response()
}
