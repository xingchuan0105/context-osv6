//! Shared smoke bootstrap (`build_smoke`) and orphan cleanup.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use super::super::{
    http_helpers::{milvus_collection_prefix_for_identity, test_auth_headers_for, unique_test_identity},
    mock_servers::{
        reset_mock_rag_state, start_mock_embedding_server, start_mock_llm_server,
        start_mock_search_server,
    },
    setup,
};
use super::TestContext;

static PG_MIGRATIONS_APPLIED: AtomicBool = AtomicBool::new(false);

async fn wait_for_worker_health(port: u16, timeout: Duration) -> anyhow::Result<()> {
    let deadline = tokio::time::Instant::now() + timeout;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()?;
    let url = format!("http://127.0.0.1:{port}/health");
    loop {
        if let Ok(resp) = client.get(&url).send().await {
            if resp.status().is_success() {
                return Ok(());
            }
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!("worker health check timed out at {url}");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn run_orphan_cleanup_once() {
    static DONE: OnceLock<()> = OnceLock::new();
    if DONE.get().is_none() {
        if let Err(e) = setup::cleanup_orphaned_test_containers().await {
            eprintln!("[product_e2e] orphan cleanup failed: {e}");
        }
        let _ = DONE.set(());
    }
}

impl TestContext {
    pub(crate) async fn resolve_use_real_search(use_real_llm: bool) -> bool {
        if !super::super::llm_real::has_real_search_credentials() {
            return false;
        }
        super::super::llm_real::ensure_search_defaults();

        if std::env::var("SEARCH_FORCE_MOCK")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            eprintln!("[product_e2e] SEARCH_FORCE_MOCK set — using mock search");
            return false;
        }

        if !use_real_llm {
            return std::env::var("SEARCH_USE_REAL")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
        }

        super::super::llm_real::load_env_from_repo_dotenv();
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
        run_orphan_cleanup_once().await;

        let (redis_url, redis_container_name) = if start_redis_container {
            let (url, name) = setup::start_redis()
                .await
                .expect("start redis for embedding cache test");
            (Some(url), Some(name))
        } else {
            (redis_url, None)
        };

        let identity = identity.or_else(|| Some(unique_test_identity()));
        let (org_id, _user_id) = identity
            .as_ref()
            .expect("identity is always Some after .or_else above");
        let milvus_collection_prefix =
            enable_rag.then(|| milvus_collection_prefix_for_identity(org_id));

        let (pg_url, shared_pg) = setup::acquire_shared_postgres()
            .await
            .expect("start shared postgres");

        let (milvus_url, shared_milvus) = if enable_rag {
            let (url, shared) = setup::acquire_shared_milvus()
                .await
                .expect("start shared milvus");
            (Some(url), Some(shared))
        } else {
            (None, None)
        };

        let object_store_dir = setup::create_temp_object_store();
        let object_root = object_store_dir.path().to_string_lossy().to_string();

        let (mock_llm_url, mock_llm_abort) = if use_real_llm {
            (String::new(), None)
        } else {
            reset_mock_rag_state();
            let (url, abort) = start_mock_llm_server().await;
            (url, Some(abort))
        };

        let (mock_search_url, mock_search_abort, search_should_429) =
            start_mock_search_server().await;

        let has_real_search = Self::resolve_use_real_search(use_real_llm).await;

        let (mock_embedding_url, mock_embedding_abort, embedding_should_503, embedding_call_count) =
            if enable_rag && !use_real_llm {
                let (url, abort, flag, call_count) = start_mock_embedding_server().await;
                (Some(url), Some(abort), Some(flag), Some(call_count))
            } else {
                (None, None, None, None)
            };

        unsafe {
            std::env::set_var("E2E_ENABLED", "true");
            std::env::set_var("DATABASE_URL", &pg_url);
            let run_migrations = !PG_MIGRATIONS_APPLIED.swap(true, Ordering::SeqCst);
            std::env::set_var(
                "AVRAG_RUN_MIGRATIONS",
                if run_migrations { "true" } else { "false" },
            );
            std::env::set_var("AVRAG_OBJECT_ROOT", &object_root);
            std::env::set_var(
                "AVRAG_ENABLE_RAG",
                if enable_rag { "true" } else { "false" },
            );
            let redis = redis_url
                .clone()
                .unwrap_or_else(|| "redis://127.0.0.1:1".to_string());
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

            if !has_real_search {
                std::env::set_var("SEARCH_PROVIDER", "brave_llm_context");
                std::env::set_var("SEARCH_BASE_URL", &mock_search_url);
                std::env::set_var("SEARCH_API_KEY", "mock");
            }

            if let Some(ref url) = mock_embedding_url {
                std::env::set_var("EMBEDDING_BASE_URL", url);
                std::env::set_var("EMBEDDING_API_KEY", "mock");
                std::env::set_var("EMBEDDING_MODEL", "mock-embedding");
                std::env::set_var("EMBEDDING_DIMENSIONS", "1024");
                std::env::set_var("AVRAG_EMBEDDING_DIM", "1024");
                std::env::set_var("MM_EMBEDDING_BASE_URL", url);
                std::env::set_var("MM_EMBEDDING_API_KEY", "mock");
                std::env::set_var(
                    "MM_EMBEDDING_MODEL",
                    "tongyi-embedding-vision-plus-2026-03-06",
                );
                std::env::set_var(
                    "MM_EMBEDDING_API_STYLE",
                    "dashscope_multimodal_embedding",
                );
                std::env::set_var("MM_EMBEDDING_DIMENSIONS", "1024");
                std::env::set_var("MILVUS_MULTIMODAL_VECTOR_DIM", "1024");
            }
        }

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
            .env("AVRAG_WORKER_POLL_MILLIS", "200")
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
                    .env("EMBEDDING_MODEL", "mock-embedding")
                    .env("EMBEDDING_DIMENSIONS", "1024")
                    .env("AVRAG_EMBEDDING_DIM", "1024")
                    .env("MM_EMBEDDING_BASE_URL", url)
                    .env("MM_EMBEDDING_API_KEY", "mock")
                    .env(
                        "MM_EMBEDDING_MODEL",
                        "tongyi-embedding-vision-plus-2026-03-06",
                    )
                    .env("MM_EMBEDDING_API_STYLE", "dashscope_multimodal_embedding")
                    .env("MM_EMBEDDING_DIMENSIONS", "1024")
                    .env("MILVUS_MULTIMODAL_VECTOR_DIM", "1024");
            }
        }

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

        wait_for_worker_health(8081, Duration::from_secs(10))
            .await
            .expect("worker health ready");

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
}
