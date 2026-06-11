//! TestContext bootstrap and HTTP helpers for Product E2E.

use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use super::{
    ChatResponse, DocumentStatus, HttpResponse, NotebookInner, NotebookResponse, SseEvent,
    SseParser, UploadResponse,
    mock_servers::{
        reset_mock_rag_state, set_mock_emit_memory_tool, set_mock_rag_codegen_chunk_id,
        set_mock_rag_codegen_chunk_ids, set_mock_rag_codegen_doc_id, set_mock_rag_codegen_query,
        set_mock_rag_multiround_profile, set_mock_rag_skill_request_memory, set_mock_rag_skip_codegen,
        start_mock_embedding_server,
        start_mock_llm_server, start_mock_search_server,
    },
    setup,
};

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

// ---------------------------------------------------------------------------
// TestContext
// ---------------------------------------------------------------------------

/// Parameters for a streaming chat request.
pub struct ChatStreamParams<'a> {
    pub query: &'a str,
    pub agent_type: &'a str,
    pub notebook_id: &'a str,
    pub doc_scope: &'a [String],
    pub session_id: Option<&'a str>,
    pub format_hint: Option<&'a str>,
    /// When true, enables `DebugTrace` events (e.g. `prompt_snapshot`) in the SSE stream.
    pub debug: bool,
}

/// Per-test execution context.
///
/// Created via `TestContext::new_smoke().await` or `new_smoke_with_rag().await`.
/// Automatically cleans up on drop (containers, temp dirs, worker process, HTTP server, mock servers).
pub struct TestContext {
    pub http_client: reqwest::Client,
    pub base_url: String,
    shared_pg: Option<Arc<setup::SharedPostgres>>,
    shared_milvus: Option<Arc<setup::SharedMilvus>>,
    milvus_collection_prefix: Option<String>,
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
    embedding_call_count: Option<Arc<AtomicUsize>>,
    redis_container_name: Option<String>,
    worker_log_path: Option<std::path::PathBuf>,
    /// Fixed per-`TestContext` run id so observability and llm_real artifacts share one directory prefix.
    artifact_run_id: String,
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
    (Uuid::new_v4().to_string(), Uuid::new_v4().to_string())
}

fn milvus_collection_prefix_for_identity(org_id: &str, _user_id: &str) -> String {
    let suffix = org_id
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .take(8)
        .collect::<String>()
        .to_ascii_lowercase();
    format!("avrag_e2e_{suffix}")
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
        Self::build_smoke(false, 300, None, false, None, false).await
    }

    /// Create a Smoke E2E context with RAG enabled (Milvus + mock embedding/LLM).
    pub async fn new_smoke_with_rag() -> Self {
        Self::build_smoke(true, 300, None, false, None, false).await
    }

    /// Create a Smoke E2E context with RAG and a custom worker per-task timeout.
    pub async fn new_smoke_with_rag_and_timeout(worker_timeout_secs: u64) -> Self {
        Self::build_smoke(true, worker_timeout_secs, None, false, None, false).await
    }

    /// Create a Smoke E2E context with a specific org/user identity (no RAG).
    pub async fn new_smoke_with_org(org_id: &str, user_id: &str) -> Self {
        let identity = Some((org_id.to_string(), user_id.to_string()));
        Self::build_smoke(false, 300, identity, false, None, false).await
    }

    /// Create a Smoke E2E context with RAG and a specific org/user identity.
    ///
    /// Used by multi-tenant isolation tests to construct a second client
    /// in a different org and verify that one org cannot read another org's data.
    pub async fn new_smoke_with_rag_and_org(org_id: &str, user_id: &str) -> Self {
        let identity = Some((org_id.to_string(), user_id.to_string()));
        Self::build_smoke(true, 300, identity, false, None, false).await
    }

    /// Embedding-cache profile: real Redis + mock embedding call counter.
    pub async fn new_embedding_cache() -> Self {
        Self::build_smoke(true, 300, None, false, None, true).await
    }

    /// Decide whether to wire real Brave Search or the local mock server.
    ///
    /// For real-LLM tests we probe connectivity first so flaky/offline networks
    /// fall back to mock search instead of producing degrade traces.
    async fn resolve_use_real_search(use_real_llm: bool) -> bool {
        if !super::llm_real::has_real_search_credentials() {
            return false;
        }
        super::llm_real::ensure_search_defaults();

        if std::env::var("SEARCH_FORCE_MOCK")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            eprintln!("[product_e2e] SEARCH_FORCE_MOCK set — using mock search");
            return false;
        }

        if !use_real_llm {
            return true;
        }

        // A prior test may have pointed SEARCH_BASE_URL at the local mock server.
        super::llm_real::load_env_from_repo_dotenv();
        avrag_search::sync_resolved_proxy_env();

        let base = std::env::var("SEARCH_BASE_URL")
            .unwrap_or_else(|_| "https://api.search.brave.com".to_string());
        let api_key = std::env::var("SEARCH_API_KEY").unwrap_or_default();
        let url = format!("{}/res/v1/llm/context", base.trim_end_matches('/'));

        let mut client_builder = reqwest::Client::builder().timeout(Duration::from_secs(8));
        if let Some(proxy_url) = avrag_search::resolved_proxy_url() {
            if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
                client_builder = client_builder.proxy(proxy);
            }
        }
        let client = match client_builder.build() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[product_e2e] Brave Search probe client build failed: {e}");
                return false;
            }
        };

        let reachable = match client
            .post(&url)
            .header("X-Subscription-Token", &api_key)
            .json(&serde_json::json!({ "q": "ping" }))
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                status.is_success() || status.as_u16() == 401 || status.as_u16() == 403
            }
            Err(e) => {
                eprintln!("[product_e2e] Brave Search probe failed: {e}");
                false
            }
        };

        if reachable {
            eprintln!("[product_e2e] using real Brave Search at {base}");
            true
        } else {
            let require_real = std::env::var("SEARCH_REQUIRE_REAL")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            if require_real {
                panic!(
                    "SEARCH_REQUIRE_REAL=1 but Brave Search unreachable at {url} — \
                     fix network/proxy or unset SEARCH_REQUIRE_REAL for mock fallback"
                );
            }
            eprintln!(
                "[product_e2e] WARN: Brave Search unreachable at {url} — \
                 falling back to mock search (set SEARCH_REQUIRE_REAL=1 to fail instead)"
            );
            false
        }
    }

    pub(crate) async fn build_smoke(
        enable_rag: bool,
        worker_timeout_secs: u64,
        identity: Option<(String, String)>,
        use_real_llm: bool,
        redis_url: Option<String>,
        start_redis_container: bool,
    ) -> Self {
        // 0. Best-effort: clean up orphan containers from previous test runs.
        //    Run at most once per process to avoid races where parallel tests
        //    remove each other's still-in-use containers.
        //    Failures are non-fatal — log and continue.
        run_orphan_cleanup_once().await;

        // Start Redis only after orphan cleanup so we do not delete a container
        // we just created (embedding-cache profile).
        let (redis_url, redis_container_name) = if start_redis_container {
            let (url, name) = setup::start_redis()
                .await
                .expect("start redis for embedding cache test");
            (Some(url), Some(name))
        } else {
            (redis_url, None)
        };

        // Each context gets a unique (org, user) pair by default so that
        // parallel tests do not share the same rate-limit bucket and
        // trigger 429s. Tests that want to share a bucket (e.g. the
        // cross-org isolation test) pass an explicit `identity`.
        let identity = identity.or_else(|| Some(unique_test_identity()));
        let (org_id, user_id) = identity
            .as_ref()
            .expect("identity is always Some after .or_else above");
        let milvus_collection_prefix =
            enable_rag.then(|| milvus_collection_prefix_for_identity(org_id, user_id));

        // 1. Acquire shared Postgres (one container per test binary).
        let (pg_url, shared_pg) = setup::acquire_shared_postgres()
            .await
            .expect("start shared postgres");

        // 2. Start Milvus if RAG enabled
        let (milvus_url, shared_milvus) = if enable_rag {
            let (url, shared) = setup::acquire_shared_milvus()
                .await
                .expect("start shared milvus");
            (Some(url), Some(shared))
        } else {
            (None, None)
        };

        // 3. Temp object store
        let object_store_dir = setup::create_temp_object_store();
        let object_root = object_store_dir.path().to_string_lossy().to_string();

        // 4. Start mock LLM unless we're using a real LLM.
        let (mock_llm_url, mock_llm_abort) = if use_real_llm {
            (String::new(), None)
        } else {
            reset_mock_rag_state();
            let (url, abort) = start_mock_llm_server().await;
            (url, Some(abort))
        };

        // 5. Start mock Search (always — used when real Brave is unavailable)
        let (mock_search_url, mock_search_abort, search_should_429) =
            start_mock_search_server().await;

        let has_real_search = Self::resolve_use_real_search(use_real_llm).await;

        // 6. Start mock Embedding if RAG enabled and not using real LLM.
        let (mock_embedding_url, mock_embedding_abort, embedding_should_503, embedding_call_count) =
            if enable_rag && !use_real_llm {
                let (url, abort, flag, call_count) = start_mock_embedding_server().await;
                (Some(url), Some(abort), Some(flag), Some(call_count))
            } else {
                (None, None, None, None)
            };

        // 5. Set env vars for AppConfig
        unsafe {
            std::env::set_var("E2E_ENABLED", "true");
            std::env::set_var("DATABASE_URL", &pg_url);
            std::env::set_var("AVRAG_RUN_MIGRATIONS", "true");
            std::env::set_var("AVRAG_OBJECT_ROOT", &object_root);
            std::env::set_var(
                "AVRAG_ENABLE_RAG",
                if enable_rag { "true" } else { "false" },
            );
            let redis = redis_url
                .clone()
                .unwrap_or_else(|| "redis://127.0.0.1:1".to_string()); // blackhole disables cache
            std::env::set_var("REDIS_URL", &redis);
            std::env::set_var("AVRAG_PUBLIC_BASE_URL", "http://127.0.0.1:8080");

            if let Some(ref url) = milvus_url {
                std::env::set_var("MILVUS_URL", url);
                std::env::set_var("MILVUS_TOKEN", "");
                std::env::set_var("MILVUS_DATABASE", "default");
                let prefix = milvus_collection_prefix
                    .as_ref()
                    .expect("RAG contexts must have a Milvus prefix");
                std::env::set_var("MILVUS_COLLECTION_PREFIX", prefix);
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

            // Search config: real Brave when credentials + connectivity allow,
            // otherwise fall back to the mock search server.
            if !has_real_search {
                std::env::set_var("SEARCH_PROVIDER", "brave_llm_context");
                std::env::set_var("SEARCH_BASE_URL", &mock_search_url);
                std::env::set_var("SEARCH_API_KEY", "mock");
            }

            if let Some(ref url) = mock_embedding_url {
                std::env::set_var("EMBEDDING_BASE_URL", url);
                std::env::set_var("EMBEDDING_API_KEY", "mock");
                std::env::set_var("EMBEDDING_MODEL", "mock-embedding");
            }
            // When use_real_llm is true, EMBEDDING_* is already set from .env above.
        }

        // 6. Bootstrap AppState and start HTTP server
        let config = app::AppConfig::from_env();
        let state = app::AppState::bootstrap(config.clone())
            .await
            .expect("bootstrap AppState");

        let router = transport_http::build_router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
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
        let worker_binary = setup::find_worker_binary()
            .await
            .expect("find worker binary");
        let worker_log_path = object_store_dir.path().join("worker.log");
        let mut cmd = tokio::process::Command::new(&worker_binary);
        cmd.env("E2E_ENABLED", "true")
            .env("DATABASE_URL", &pg_url)
            .env("AVRAG_RUN_MIGRATIONS", "false")
            .env("AVRAG_OBJECT_ROOT", &object_root)
            .env(
                "AVRAG_ENABLE_RAG",
                if enable_rag { "true" } else { "false" },
            )
            .env(
                "REDIS_URL",
                redis_url.as_deref().unwrap_or("redis://127.0.0.1:1"),
            )
            .env("AVRAG_PUBLIC_BASE_URL", &base_url)
            .env("AVRAG_WORKER_ID", "test-worker")
            .env("AVRAG_WORKER_POLL_SECS", "1")
            .env(
                "AVRAG_INGESTION_TASK_TIMEOUT_SECS",
                worker_timeout_secs.to_string(),
            )
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

        if let Some(ref url) = milvus_url {
            let prefix = milvus_collection_prefix.clone().unwrap_or_default();
            cmd.env("MILVUS_URL", url)
                .env("MILVUS_TOKEN", "")
                .env("MILVUS_DATABASE", "default")
                .env("MILVUS_COLLECTION_PREFIX", prefix);
        }
        // Worker LLM env: real or mock depending on mode.
        if use_real_llm {
            for key in [
                "AGENT_LLM_BASE_URL",
                "AGENT_LLM_API_KEY",
                "AGENT_LLM_MODEL",
                "MEMORY_LLM_BASE_URL",
                "MEMORY_LLM_API_KEY",
                "MEMORY_LLM_MODEL",
                "INGESTION_LLM_BASE_URL",
                "INGESTION_LLM_API_KEY",
                "INGESTION_LLM_MODEL",
                "EMBEDDING_BASE_URL",
                "EMBEDDING_API_KEY",
                "EMBEDDING_MODEL",
                "EMBEDDING_DIMENSIONS",
                "AVRAG_EMBEDDING_DIM",
                "OFFICE_PARSER_BASE_URL",
                "PDF_RENDERER_BASE_URL",
                "PDF_VISUAL_PAGES_PER_CHUNK",
                "INGESTION_PDF_MAX_PAGES",
                "INGESTION_TRIPLET_ENABLED",
                "INGESTION_VLM_TRIPLET_ENABLED",
                "INGESTION_VLM_SUMMARY_ENABLED",
                "INGESTION_TRIPLET_MIN_CONFIDENCE",
                "DASHSCOPE_API_KEY",
                "MM_EMBEDDING_BASE_URL",
                "MM_EMBEDDING_API_KEY",
                "MM_EMBEDDING_MODEL",
                "MM_EMBEDDING_API_STYLE",
                "MM_EMBEDDING_DIMENSIONS",
                "MM_RERANK_BASE_URL",
                "MM_RERANK_API_KEY",
                "MM_RERANK_MODEL",
                "MM_RERANK_API_STYLE",
            ] {
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

        // Worker Search env: real if available, otherwise mock.
        if has_real_search {
            avrag_search::sync_resolved_proxy_env();
            for key in [
                "SEARCH_PROVIDER",
                "SEARCH_BASE_URL",
                "SEARCH_API_KEY",
                "HTTPS_PROXY",
                "https_proxy",
                "HTTP_PROXY",
                "http_proxy",
            ] {
                if let Ok(v) = std::env::var(key) {
                    cmd.env(key, v);
                }
            }
        } else {
            cmd.env("SEARCH_PROVIDER", "brave_llm_context")
                .env("SEARCH_BASE_URL", &mock_search_url)
                .env("SEARCH_API_KEY", "mock");
        }

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
                let mut file = match tokio::fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(&log_path)
                    .await
                {
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

        // Real LLM + thinking mode + web search can exceed the smoke default.
        let http_timeout_secs = if use_real_llm { 180 } else { 60 };
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(http_timeout_secs))
            .default_headers(match &identity {
                Some((org, user)) => test_auth_headers_for(org, user),
                None => unreachable!("identity is always Some after .or_else above"),
            })
            .build()
            .expect("reqwest client build");

        let now = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let short_commit = option_env!("GITHUB_SHA")
            .map(|s| &s[..s.len().min(8)])
            .unwrap_or("local");
        let artifact_run_id = format!("e2e_{now}_{short_commit}_{}", Uuid::new_v4().simple());

        Self {
            http_client: client,
            base_url,
            shared_pg: Some(shared_pg),
            shared_milvus,
            milvus_collection_prefix,
            worker: Some(worker),
            server_abort: Some(abort_tx),
            object_store_dir,
            pg_url,
            mock_llm_abort,
            mock_embedding_abort,
            mock_search_abort: Some(mock_search_abort),
            search_should_429: Some(search_should_429),
            embedding_should_503,
            embedding_call_count,
            redis_container_name,
            worker_log_path: Some(worker_log_path),
            artifact_run_id,
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
        self.upload_document_to_notebook(fixture, &notebook.id)
            .await
    }

    /// Upload a fixture file to an existing notebook.
    pub async fn upload_document_to_notebook(
        &self,
        fixture: &str,
        notebook_id: &str,
    ) -> anyhow::Result<UploadResponse> {
        let content = setup::load_fixture(fixture)?;
        self.upload_bytes_to_notebook(fixture, content.into_bytes(), notebook_id)
            .await
    }

    /// Upload a local file (absolute or relative path) to an existing notebook.
    pub async fn upload_file_from_path_to_notebook(
        &self,
        file_path: &str,
        notebook_id: &str,
    ) -> anyhow::Result<UploadResponse> {
        let path = std::path::Path::new(file_path);
        let bytes = std::fs::read(path)
            .map_err(|e| anyhow::anyhow!("read {}: {e}", path.display()))?;
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("document.bin");
        self.upload_bytes_to_notebook(filename, bytes, notebook_id)
            .await
    }

    /// Upload a local file into a freshly created notebook.
    pub async fn upload_file_from_path(&self, file_path: &str) -> anyhow::Result<UploadResponse> {
        let notebook = self.create_notebook("test-notebook").await?;
        self.upload_file_from_path_to_notebook(file_path, &notebook.id)
            .await
    }

    async fn upload_bytes_to_notebook(
        &self,
        filename: &str,
        bytes: Vec<u8>,
        notebook_id: &str,
    ) -> anyhow::Result<UploadResponse> {
        let mime_type = setup::mime_type_for_filename(filename);

        let resp = self
            .http_client
            .post(format!(
                "{}/api/v1/notebooks/{}/documents",
                self.base_url, notebook_id
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
                anyhow::bail!(
                    "wait_for_ingestion timed out after {timeout:?}, last status={last_status}"
                );
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// GET `/api/v1/documents/{id}/status` (for diagnostics).
    pub async fn fetch_document_status(&self, doc_id: &str) -> anyhow::Result<serde_json::Value> {
        self.fetch_status_with_retry(doc_id).await
    }

    /// Last N lines of the worker log (when ingestion or RAG fails).
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

    /// GET the document status with up to 3 retries on transient errors.
    /// Returns the parsed JSON body on success.
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

    /// Send a RAG chat query and return the raw HTTP response.
    pub async fn chat(
        &self,
        query: &str,
        notebook_id: &str,
        doc_scope: &[String],
    ) -> anyhow::Result<HttpResponse> {
        self.post_rag_chat(query, notebook_id, doc_scope, None, true)
            .await
    }

    /// RAG chat without pinning mock synthesis chunk ids (exercises real bridge retrieval).
    pub async fn chat_without_mock_chunk_pin(
        &self,
        query: &str,
        notebook_id: &str,
        doc_scope: &[String],
    ) -> anyhow::Result<HttpResponse> {
        self.post_rag_chat(query, notebook_id, doc_scope, None, false)
            .await
    }

    /// Send a RAG chat query with an optional format_hint.
    pub async fn chat_with_format_hint(
        &self,
        query: &str,
        notebook_id: &str,
        doc_scope: &[String],
        format_hint: Option<&str>,
    ) -> anyhow::Result<HttpResponse> {
        self.post_rag_chat(query, notebook_id, doc_scope, format_hint, true)
            .await
    }

    async fn post_rag_chat(
        &self,
        query: &str,
        notebook_id: &str,
        doc_scope: &[String],
        format_hint: Option<&str>,
        pin_mock_chunk_ids: bool,
    ) -> anyhow::Result<HttpResponse> {
        set_mock_rag_codegen_query(query);
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

    /// RAG chat with an existing session_id for multi-turn tests.
    pub async fn chat_with_session(
        &self,
        query: &str,
        notebook_id: &str,
        doc_scope: &[String],
        session_id: &str,
    ) -> anyhow::Result<HttpResponse> {
        set_mock_rag_codegen_query(query);
        let body = serde_json::json!({
            "query": query,
            "agent_type": "rag",
            "notebook_id": notebook_id,
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

    /// Send a general/chat agent query and return the raw HTTP response.
    pub async fn chat_general(
        &self,
        query: &str,
        notebook_id: &str,
    ) -> anyhow::Result<HttpResponse> {
        let resp = self
            .http_client
            .post(format!("{}/api/v1/chat", self.base_url))
            .json(&serde_json::json!({
                "query": query,
                "agent_type": "general",
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

    /// Create a read-only share link for a notebook owned by this context.
    pub async fn create_share_token(&self, notebook_id: &str) -> anyhow::Result<String> {
        let resp = self
            .http_client
            .post(format!(
                "{}/api/v1/notebooks/{notebook_id}/share",
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

    /// Chat via a share token (collaboration mode A: read-only cross-user access).
    pub async fn chat_with_share(
        &self,
        query: &str,
        notebook_id: &str,
        share_token: &str,
    ) -> anyhow::Result<HttpResponse> {
        let resp = self
            .http_client
            .post(format!("{}/api/v1/chat", self.base_url))
            .json(&serde_json::json!({
                "query": query,
                "agent_type": "general",
                "notebook_id": notebook_id,
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

    /// Return the mock embedding HTTP call count (embedding-cache profile only).
    pub fn embedding_call_count(&self) -> usize {
        self.embedding_call_count
            .as_ref()
            .map(|c| c.load(Ordering::SeqCst))
            .unwrap_or(0)
    }

    /// Send a Search query and return the raw HTTP response.
    pub async fn search(&self, query: &str, notebook_id: &str) -> anyhow::Result<HttpResponse> {
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

    /// Send a streaming chat request and return the raw SSE event stream.
    ///
    /// Reads the stream until the response body is fully consumed
    /// (production closes the HTTP response after the `done` / `error`
    /// event). As a safety net, the function bails after `max_wait`.
    /// `max_events` is only used to bail if the stream is genuinely unbounded.
    pub async fn chat_stream_with_params(
        &self,
        params: ChatStreamParams<'_>,
        max_events: usize,
        max_wait: Duration,
    ) -> anyhow::Result<Vec<SseEvent>> {
        let mut body = serde_json::json!({
            "query": params.query,
            "agent_type": params.agent_type,
            "notebook_id": params.notebook_id,
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
        if params.agent_type == "rag" && !params.doc_scope.is_empty() {
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

    /// Streaming RAG chat (backward-compatible wrapper).
    pub async fn chat_stream(
        &self,
        query: &str,
        notebook_id: &str,
        doc_scope: &[String],
        max_events: usize,
        max_wait: Duration,
    ) -> anyhow::Result<Vec<SseEvent>> {
        self.chat_stream_with_params(
            ChatStreamParams {
                query,
                agent_type: "rag",
                notebook_id,
                doc_scope,
                session_id: None,
                format_hint: None,
                debug: false,
            },
            max_events,
            max_wait,
        )
        .await
    }

    // -----------------------------------------------------------------------
    // Failure artifact capture
    // -----------------------------------------------------------------------

    /// Query the chunk_count stored in PG for a completed document.
    pub async fn query_document_chunk_count(&self, document_id: &str) -> anyhow::Result<usize> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let doc_id = Uuid::parse_str(document_id)?;
        let row: (i32,) = sqlx::query_as("SELECT chunk_count FROM documents WHERE id = $1")
            .bind(doc_id)
            .fetch_one(&pool)
            .await?;
        Ok(row.0 as usize)
    }

    /// Text body chunks plus multimodal/visual chunks (scan PDFs may have 0 text rows).
    pub async fn query_ingested_chunk_units(&self, document_id: &str) -> anyhow::Result<usize> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let doc_id = Uuid::parse_str(document_id)?;
        let text: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM chunks WHERE document_id = $1")
                .bind(doc_id)
                .fetch_one(&pool)
                .await?;
        let multimodal: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM document_multimodal_chunks WHERE document_id = $1",
        )
        .bind(doc_id)
        .fetch_one(&pool)
        .await?;
        Ok((text.0 + multimodal.0) as usize)
    }

    /// Return one chunk id from PG for mock codegen embedding.
    pub async fn query_first_chunk_id(&self, document_id: &str) -> anyhow::Result<String> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let doc_id = Uuid::parse_str(document_id)?;
        let row: (Uuid,) =
            sqlx::query_as("SELECT id FROM chunks WHERE document_id = $1 ORDER BY created_at LIMIT 1")
                .bind(doc_id)
                .fetch_one(&pool)
                .await?;
        Ok(row.0.to_string())
    }

    /// Return all chunk ids for a document (for bridge smoke assertions).
    pub async fn query_document_chunk_ids(&self, document_id: &str) -> anyhow::Result<Vec<String>> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let doc_id = Uuid::parse_str(document_id)?;
        let rows: Vec<(Uuid,)> =
            sqlx::query_as("SELECT id FROM chunks WHERE document_id = $1 ORDER BY created_at")
                .bind(doc_id)
                .fetch_all(&pool)
                .await?;
        Ok(rows.into_iter().map(|(id,)| id.to_string()).collect())
    }

    /// Pin chunk id for mock LLM codegen stdout (RAG smoke happy path).
    pub fn set_mock_rag_chunk_id(&self, chunk_id: &str) {
        let _ = self;
        set_mock_rag_codegen_chunk_id(chunk_id);
    }

    /// Force mock LLM to skip codegen and exercise server auto_fallback.
    pub fn set_mock_rag_skip_codegen(&self, skip: bool) {
        let _ = self;
        set_mock_rag_skip_codegen(skip);
    }

    /// Enable multiround RAG codegen: doc_profile → chunk_fetch → synthesis.
    pub fn set_mock_rag_multiround_profile(&self, enabled: bool) {
        let _ = self;
        set_mock_rag_multiround_profile(enabled);
    }

    /// Pin doc_id for multiround mock codegen round0 (`doc_profile`).
    pub fn set_mock_rag_codegen_doc_id(&self, doc_id: &str) {
        let _ = self;
        set_mock_rag_codegen_doc_id(doc_id);
    }

    /// Emit a memory tool call on the next tools-enabled retrieve turn.
    pub fn set_mock_emit_memory_tool(&self, tool: Option<&str>) {
        let _ = self;
        set_mock_emit_memory_tool(tool.map(str::to_string));
    }

    /// First RAG retrieve turn returns `{"skill_request":["memory"]}` to disclose on-demand tools.
    pub fn set_mock_rag_skill_request_memory(&self, enabled: bool) {
        let _ = self;
        set_mock_rag_skill_request_memory(enabled);
    }

    /// Read the latest user message content and resolved_query for a session.
    pub async fn query_latest_user_resolved_query(
        &self,
        session_id: &str,
    ) -> anyhow::Result<(String, Option<String>)> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let sid = Uuid::parse_str(session_id)?;
        let row: (String, Option<String>) = sqlx::query_as(
            "SELECT content, resolved_query FROM chat_messages \
             WHERE session_id = $1 AND role = 'user' \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(sid)
        .fetch_one(&pool)
        .await?;
        Ok(row)
    }

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

    /// Directory for llm_real artifacts for a given test name.
    pub fn llm_real_artifact_dir(&self, test_name: &str) -> std::path::PathBuf {
        self.artifact_dir(test_name, "llm_real")
    }

    /// Save debugging artifacts on test failure.
    pub fn save_failure_artifacts(
        &self,
        test_name: &str,
        response_json: Option<&serde_json::Value>,
    ) {
        let out_dir = self.artifact_dir(test_name, "failures");
        let _ = std::fs::create_dir_all(&out_dir);

        if let Some(body) = response_json {
            let _ = std::fs::write(
                out_dir.join("response_body.json"),
                serde_json::to_string_pretty(body).unwrap_or_default(),
            );
        }

        if let Some(ref log_path) = self.worker_log_path {
            if log_path.exists() {
                let _ = std::fs::copy(log_path, out_dir.join("worker_logs.txt"));
            }
        }
    }

    fn write_reasoning_capture_files(
        out_dir: &std::path::Path,
        capture: &super::StreamReasoningCapture,
    ) {
        let _ = std::fs::write(out_dir.join("reasoning_summary.txt"), &capture.summary);

        let trace_lines: String = capture
            .trace_reasoning
            .iter()
            .filter_map(|rec| serde_json::to_string(rec).ok())
            .collect::<Vec<_>>()
            .join("\n");
        let _ = std::fs::write(out_dir.join("trace_reasoning.jsonl"), trace_lines);

        let _ = std::fs::write(
            out_dir.join("prompt_snapshots.json"),
            serde_json::to_string_pretty(&capture.prompt_snapshots).unwrap_or_default(),
        );
    }

    /// Save observability artifacts for a typed ChatResponse.
    ///
    /// When `capture` is provided, also writes reasoning files (same layout as `llm_real`).
    pub fn save_observability_artifact(
        &self,
        test_name: &str,
        resp: &ChatResponse,
        capture: Option<&super::StreamReasoningCapture>,
        extra: Option<&serde_json::Value>,
    ) {
        let out_dir = self.artifact_dir(test_name, "observability");
        let _ = std::fs::create_dir_all(&out_dir);

        let _ = std::fs::write(
            out_dir.join("response.json"),
            serde_json::to_string_pretty(resp).unwrap_or_default(),
        );

        if let Some(capture) = capture {
            Self::write_reasoning_capture_files(&out_dir, capture);
        }

        let stream_error_with_done = extra
            .and_then(|v| v.get("stream_error_with_done"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut metadata = serde_json::json!({
            "test_name": test_name,
            "run_id": self.artifact_run_id,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "degrade_trace_count": resp.degrade_trace.len(),
            "usage": resp.usage,
            "citation_count": resp.citations.len(),
            "agent_type": resp.agent_type,
            "session_id": resp.session_id,
            "message_id": resp.message_id,
            "stream_error_with_done": stream_error_with_done,
            "extra": extra.cloned().unwrap_or(serde_json::Value::Null),
        });

        if let Some(capture) = capture {
            let reasoning_empty_warning =
                capture.summary.is_empty() && capture.trace_reasoning.is_empty();
            metadata["reasoning_delta_count"] = serde_json::json!(capture.delta_count);
            metadata["reasoning_summary_chars"] =
                serde_json::json!(capture.summary.chars().count());
            metadata["reasoning_summary_present"] = serde_json::json!(!capture.summary.is_empty());
            metadata["trace_reasoning_count"] = serde_json::json!(capture.trace_reasoning.len());
            metadata["prompt_snapshot_count"] = serde_json::json!(capture.prompt_snapshots.len());
            metadata["reasoning_empty_warning"] = serde_json::json!(reasoning_empty_warning);
        }

        let _ = std::fs::write(
            out_dir.join("metadata.json"),
            serde_json::to_string_pretty(&metadata).unwrap_or_default(),
        );

        if let Some(ref log_path) = self.worker_log_path {
            if log_path.exists() {
                let _ = std::fs::copy(log_path, out_dir.join("worker_logs.txt"));
            }
        }
    }

    /// Save a real-LLM test artifact (answer, citations, metadata) so the
    /// output can be audited even when the test passes.
    ///
    /// Writes a complete artifact set under `llm_real/<run_id>/<test_name>/`:
    /// `response.json`, `reasoning_summary.txt`, `trace_reasoning.jsonl`,
    /// `prompt_snapshots.json`, and `metadata.json`.
    pub fn save_llm_artifact(
        &self,
        test_name: &str,
        resp: &ChatResponse,
        extra: Option<serde_json::Value>,
        capture: Option<super::StreamReasoningCapture>,
    ) {
        let capture = capture.unwrap_or(super::StreamReasoningCapture {
            summary: String::new(),
            delta_count: 0,
            trace_reasoning: Vec::new(),
            prompt_snapshots: Vec::new(),
        });

        let extra_value = extra.unwrap_or(serde_json::Value::Null);
        self.save_observability_artifact(test_name, resp, Some(&capture), Some(&extra_value));

        let out_dir = self.llm_real_artifact_dir(test_name);
        let _ = std::fs::create_dir_all(&out_dir);

        let _ = std::fs::write(
            out_dir.join("response.json"),
            serde_json::to_string_pretty(resp).unwrap_or_default(),
        );

        Self::write_reasoning_capture_files(&out_dir, &capture);

        let reasoning_empty_warning =
            capture.summary.is_empty() && capture.trace_reasoning.is_empty();
        let stream_error_with_done = extra_value
            .get("stream_error_with_done")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let metadata = serde_json::json!({
            "test_name": test_name,
            "run_id": self.artifact_run_id,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "usage": resp.usage,
            "degrade_trace_count": resp.degrade_trace.len(),
            "citation_count": resp.citations.len(),
            "agent_type": resp.agent_type,
            "session_id": resp.session_id,
            "message_id": resp.message_id,
            "reasoning_delta_count": capture.delta_count,
            "reasoning_summary_chars": capture.summary.chars().count(),
            "reasoning_summary_present": !capture.summary.is_empty(),
            "trace_reasoning_count": capture.trace_reasoning.len(),
            "prompt_snapshot_count": capture.prompt_snapshots.len(),
            "reasoning_empty_warning": reasoning_empty_warning,
            "stream_error_with_done": stream_error_with_done,
            "models": {
                "agent_llm": std::env::var("AGENT_LLM_MODEL").unwrap_or_default(),
                "embedding": std::env::var("EMBEDDING_MODEL").unwrap_or_default(),
            },
            "extra": extra_value,
        });
        let _ = std::fs::write(
            out_dir.join("metadata.json"),
            serde_json::to_string_pretty(&metadata).unwrap_or_default(),
        );

        if let Some(ref log_path) = self.worker_log_path {
            if log_path.exists() {
                let _ = std::fs::copy(log_path, out_dir.join("worker_logs.txt"));
            }
        }
    }

    /// Best-effort check that the worker log contains evidence that
    /// `tool_name` was called.  This is a safety-net for real-LLM tests.
    ///
    /// **NOTE**: RAG tool calls happen in the HTTP server process, not the
    /// worker process, so this assertion is currently best-effort only.
    /// It logs a warning when the tool name is missing but does not hard-fail.
    /// Future work: add a server-side log file for white-box assertions.
    pub fn assert_tool_called(&self, tool_name: &str) {
        let Some(ref log_path) = self.worker_log_path else {
            eprintln!("[assert_tool_called] no worker log path — skipping");
            return;
        };
        let content = match std::fs::read_to_string(log_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[assert_tool_called] cannot read worker log: {e}");
                return;
            }
        };

        let found = content.contains(&format!("\"tool\":\"{tool_name}\""))
            || content.contains(&format!("\"tool\": \"{tool_name}\""))
            || content.contains(&format!("tool={tool_name}"))
            || content.contains(tool_name);

        if !found {
            eprintln!(
                "[assert_tool_called] WARNING: no evidence of '{tool_name}' in worker log. \
                 (RAG tool calls run in the HTTP server, not the worker — this is expected.)"
            );
        }
    }

    /// Build the artifact directory path for a test (uses fixed `artifact_run_id`).
    fn artifact_dir(&self, test_name: &str, bucket: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("e2e_output")
            .join(bucket)
            .join(&self.artifact_run_id)
            .join(test_name)
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
        // Release shared Postgres; last context stops the container synchronously.
        if let Some(pg) = self.shared_pg.take() {
            setup::release_shared_postgres(&pg);
        }
        // Drop Milvus collections before releasing the shared instance.
        if let Some(ref prefix) = self.milvus_collection_prefix {
            setup::sync_drop_milvus_collections(prefix);
        }
        // Release shared Milvus; last context stops only test-owned containers.
        if let Some(milvus) = self.shared_milvus.take() {
            setup::release_shared_milvus(&milvus);
        }
        // Stop Redis container when started for embedding-cache profile.
        if let Some(ref container) = self.redis_container_name {
            setup::sync_stop_redis(container);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn milvus_collection_prefix_uses_context_identity_suffix() {
        let prefix = milvus_collection_prefix_for_identity(
            "12345678-aaaa-bbbb-cccc-dddddddddddd",
            "ignored-user",
        );

        assert_eq!(prefix, "avrag_e2e_12345678");
    }
}
