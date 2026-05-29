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

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use axum::{response::IntoResponse, routing::post, Json, Router};
use serde_json::json;

// ---------------------------------------------------------------------------
// HTTP raw response (protocol layer)
// ---------------------------------------------------------------------------

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
    worker: Option<tokio::process::Child>,
    server_abort: Option<tokio::sync::oneshot::Sender<()>>,
    #[allow(dead_code)]
    object_store_dir: tempfile::TempDir,
    mock_llm_abort: Option<tokio::sync::oneshot::Sender<()>>,
    mock_embedding_abort: Option<tokio::sync::oneshot::Sender<()>>,
    mock_search_abort: Option<tokio::sync::oneshot::Sender<()>>,
    search_should_429: Option<Arc<AtomicBool>>,
}

/// Default auth headers for test requests.
fn test_auth_headers() -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("x-org-id", "00000000-0000-0000-0000-000000000001".parse().unwrap());
    headers.insert("x-user-id", "00000000-0000-0000-0000-000000000001".parse().unwrap());
    headers.insert("x-permissions", "external_network".parse().unwrap());
    headers
}

impl TestContext {
    /// Create a Smoke E2E context (no RAG).
    pub async fn new_smoke() -> Self {
        Self::build_smoke(false).await
    }

    /// Create a Smoke E2E context with RAG enabled (Milvus + mock embedding/LLM).
    pub async fn new_smoke_with_rag() -> Self {
        Self::build_smoke(true).await
    }

    async fn build_smoke(enable_rag: bool) -> Self {
        // 1. Start Postgres
        let pg_url = setup::start_postgres().await.expect("start postgres");
        let pg_container_name = format!("avrag-test-pg-{}", pg_url.rsplit(':').next().unwrap());

        // 2. Start Milvus if RAG enabled
        let (milvus_url, milvus_container_name) = if enable_rag {
            let url = setup::start_milvus().await.expect("start milvus");
            let name = format!("avrag-test-milvus-{}", url.rsplit(':').next().unwrap());
            (Some(url), Some(name))
        } else {
            (None, None)
        };

        // 3. Temp object store
        let object_store_dir = setup::create_temp_object_store();
        let object_root = object_store_dir.path().to_string_lossy().to_string();

        // 4. Start mock LLM (always — needed by Search and RAG)
        let (mock_llm_url, mock_llm_abort) = start_mock_llm_server().await;

        // 5. Start mock Search (always — needed by Search tests)
        let (mock_search_url, mock_search_abort, search_should_429) = start_mock_search_server().await;

        // 6. Start mock Embedding if RAG enabled
        let (mock_embedding_url, mock_embedding_abort) = if enable_rag {
            let (url, abort) = start_mock_embedding_server().await;
            (Some(url), Some(abort))
        } else {
            (None, None)
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
            // LLM config (always)
            std::env::set_var("AGENT_LLM_BASE_URL", &mock_llm_url);
            std::env::set_var("AGENT_LLM_API_KEY", "mock");
            std::env::set_var("AGENT_LLM_MODEL", "mock-llm");
            std::env::set_var("MEMORY_LLM_BASE_URL", &mock_llm_url);
            std::env::set_var("MEMORY_LLM_API_KEY", "mock");
            std::env::set_var("MEMORY_LLM_MODEL", "mock-llm");
            std::env::set_var("INGESTION_LLM_BASE_URL", &mock_llm_url);
            std::env::set_var("INGESTION_LLM_API_KEY", "mock");
            std::env::set_var("INGESTION_LLM_MODEL", "mock-llm");

            // Search config (always)
            std::env::set_var("SEARCH_PROVIDER", "brave_llm_context");
            std::env::set_var("SEARCH_BASE_URL", &mock_search_url);
            std::env::set_var("SEARCH_API_KEY", "mock");

            if let Some(ref url) = mock_embedding_url {
                std::env::set_var("EMBEDDING_BASE_URL", url);
                std::env::set_var("EMBEDDING_API_KEY", "mock");
                std::env::set_var("EMBEDDING_MODEL", "mock-embedding");
            }
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
        let mut cmd = tokio::process::Command::new(&worker_binary);
        cmd.env("DATABASE_URL", &pg_url)
            .env("AVRAG_RUN_MIGRATIONS", "false")
            .env("AVRAG_OBJECT_ROOT", &object_root)
            .env("AVRAG_ENABLE_RAG", if enable_rag { "true" } else { "false" })
            .env("REDIS_URL", "")
            .env("AVRAG_PUBLIC_BASE_URL", &base_url)
            .env("AVRAG_WORKER_ID", "test-worker")
            .env("AVRAG_WORKER_POLL_SECS", "1")
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true);

        if let Some(ref url) = milvus_url {
            cmd.env("MILVUS_URL", url)
               .env("MILVUS_TOKEN", "")
               .env("MILVUS_DATABASE", "default")
               .env("MILVUS_COLLECTION_PREFIX", std::env::var("MILVUS_COLLECTION_PREFIX").unwrap_or_default());
        }
        // Worker always gets LLM + Search env vars
        cmd.env("AGENT_LLM_BASE_URL", &mock_llm_url)
           .env("AGENT_LLM_API_KEY", "mock")
           .env("AGENT_LLM_MODEL", "mock-llm")
           .env("MEMORY_LLM_BASE_URL", &mock_llm_url)
           .env("MEMORY_LLM_API_KEY", "mock")
           .env("MEMORY_LLM_MODEL", "mock-llm")
           .env("INGESTION_LLM_BASE_URL", &mock_llm_url)
           .env("INGESTION_LLM_API_KEY", "mock")
           .env("INGESTION_LLM_MODEL", "mock-llm")
           .env("SEARCH_PROVIDER", "brave_llm_context")
           .env("SEARCH_BASE_URL", &mock_search_url)
           .env("SEARCH_API_KEY", "mock");

        if let Some(ref url) = mock_embedding_url {
            cmd.env("EMBEDDING_BASE_URL", url)
               .env("EMBEDDING_API_KEY", "mock")
               .env("EMBEDDING_MODEL", "mock-embedding");
        }

        let worker = cmd.spawn().expect("spawn worker");

        // Give worker a moment to start
        tokio::time::sleep(Duration::from_secs(1)).await;

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .default_headers(test_auth_headers())
            .build()
            .expect("reqwest client build");

        Self {
            http_client: client,
            base_url,
            pg_container_name,
            milvus_container_name,
            worker: Some(worker),
            server_abort: Some(abort_tx),
            object_store_dir,
            mock_llm_abort: Some(mock_llm_abort),
            mock_embedding_abort,
            mock_search_abort: Some(mock_search_abort),
            search_should_429: Some(search_should_429),
        }
    }

    /// Create an Integration context.
    pub async fn new_integration() -> Self {
        Self::new_smoke().await
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
        if status != 202 && status != 201 {
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
            status: 202,
        })
    }

    /// Poll ingestion status until completed, failed, or timeout.
    pub async fn wait_for_ingestion(
        &self,
        doc_id: &str,
        timeout: Duration,
    ) -> anyhow::Result<DocumentStatus> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut last_status = String::new();
        loop {
            let resp = self
                .http_client
                .get(format!("{}/api/v1/documents/{doc_id}/status", self.base_url))
                .send()
                .await?;
            let body = resp.json::<serde_json::Value>().await?;
            let status = body["status"].as_str().unwrap_or("unknown").to_string();
            if status != last_status {
                eprintln!("[wait_for_ingestion] doc={doc_id} status={status}");
                last_status = status.clone();
            }
            match status.as_str() {
                "completed" | "ready" => return Ok(DocumentStatus::Completed),
                "failed" | "error" => return Ok(DocumentStatus::Failed),
                _ => {}
            }
            if tokio::time::Instant::now() > deadline {
                anyhow::bail!("wait_for_ingestion timed out after {timeout:?}, last status={last_status}");
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// Send a RAG chat query and return the raw HTTP response.
    pub async fn chat(
        &self,
        query: &str,
        notebook_id: &str,
        doc_scope: &[String],
    ) -> anyhow::Result<HttpResponse> {
        let resp = self
            .http_client
            .post(format!("{}/api/v1/chat", self.base_url))
            .json(&serde_json::json!({
                "query": query,
                "agent_type": "rag",
                "notebook_id": notebook_id,
                "doc_scope": doc_scope,
                "stream": false,
            }))
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

    // -----------------------------------------------------------------------
    // Failure artifact capture
    // -----------------------------------------------------------------------

    /// Toggle mock search server to return 429 (rate limit).
    pub fn set_search_429(&self, value: bool) {
        if let Some(ref flag) = self.search_should_429 {
            flag.store(value, Ordering::SeqCst);
        }
    }

    /// Save debugging artifacts on test failure.
    pub fn save_failure_artifacts(&self, _test_name: &str) {
        // TODO(Phase 4): implement
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
        // Stop Milvus container — fire-and-forget
        if let Some(ref container) = self.milvus_container_name {
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

/// Start a mock LLM HTTP server on an ephemeral port.
///
/// Returns (base_url, abort_sender).
async fn start_mock_llm_server() -> (String, tokio::sync::oneshot::Sender<()>) {
    let app = Router::new()
        .route("/chat/completions", post(mock_llm_handler));

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
/// Returns (base_url, abort_sender).
async fn start_mock_embedding_server() -> (String, tokio::sync::oneshot::Sender<()>) {
    let app = Router::new()
        .route("/embeddings", post(mock_embedding_handler));

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

    (base_url, abort_tx)
}

async fn mock_llm_handler(Json(req): Json<serde_json::Value>) -> Json<serde_json::Value> {
    let messages = req["messages"].as_array().cloned().unwrap_or_default();
    let system_prompt = messages
        .first()
        .and_then(|m| m["content"].as_str())
        .unwrap_or("");
    let user_prompt = messages
        .get(1)
        .and_then(|m| m["content"].as_str())
        .unwrap_or("");

    let content = if system_prompt.contains("Context OS RAG retrieval planner") {
        r#"{"calls": [{"tool": "dense_retrieval", "version": "1.0", "args": {"queries": ["antifragility Taleb summary"], "modality": "text", "top_k": 10}}], "next_step": "answer"}"#
    } else if system_prompt.contains("Context OS retrieval coverage evaluator") {
        r#"{"decision": "sufficient", "dimensions": [{"name": "coverage", "attempted": true, "covered": true, "retrieved_count": 3, "query_ids": ["q1"], "status": "covered_strong"}], "next_actions": [], "reasoning": "good"}"#
    } else if system_prompt.contains("Context OS RAG answer agent") {
        "Based on the document, antifragility is a property of systems that increase in capability, resilience, or robustness as a result of stressors, shocks, volatility, noise, mistakes, faults, attacks, or failures. The concept was developed by Nassim Nicholas Taleb."
    } else if system_prompt.contains("Context OS Web Search planner") {
        r#"{"sub_queries": ["Tokyo weather today"], "intent_summary": "The user wants to know the current weather in Tokyo.", "needs_clarification": false}"#
    } else if system_prompt.contains("Context OS web-search coverage evaluator") {
        r#"{"decision": "sufficient", "dimensions": [{"name": "coverage", "attempted": true, "covered": true, "retrieved_count": 1, "query_ids": ["q1"], "status": "covered_strong"}], "next_actions": [], "reasoning": "good"}"#
    } else if system_prompt.contains("Answer the user's original web-search question") || user_prompt.contains("Search results:") {
        "The weather in Tokyo today is sunny with a high of 25°C [[1]]."
    } else {
        // Summary generation fallback
        "This document discusses antifragility, a concept by Nassim Nicholas Taleb describing systems that benefit from shock and disorder."
    };

    Json(json!({
        "choices": [{"message": {"role": "assistant", "content": content}}],
        "usage": {"prompt_tokens": 100, "completion_tokens": 50, "total_tokens": 150},
        "model": "mock-llm"
    }))
}

async fn mock_embedding_handler(Json(req): Json<serde_json::Value>) -> Json<serde_json::Value> {
    let texts = req["input"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();

    let dim = req["dimensions"].as_u64().unwrap_or(1024) as usize;
    // All vectors identical so dense retrieval always returns high similarity.
    let vec: Vec<f32> = (0..dim).map(|j| 0.1_f32 + (j % 10) as f32 * 0.01).collect();
    let data: Vec<serde_json::Value> = texts.iter().map(|_| json!({"embedding": vec})).collect();

    Json(json!({ "data": data, "model": "mock-embedding" }))
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
