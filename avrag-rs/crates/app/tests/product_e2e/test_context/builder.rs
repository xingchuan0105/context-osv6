//! Shared smoke bootstrap (`build_smoke`) and orphan cleanup.

use std::hash::{Hash, Hasher};
use std::io::Write;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use super::super::{
    http_helpers::{
        milvus_collection_prefix_for_identity, test_auth_headers_for, unique_test_identity,
    },
    mock_servers::{
        reset_mock_rag_state, start_mock_embedding_server, start_mock_llm_server,
        start_mock_office_parser_server, start_mock_paddle_ocr_server, start_mock_search_server,
    },
    persistent_runtime::{bind_persistent_listener, spawn_persistent},
    setup,
};
use super::TestContext;
use super::config::E2eBootstrapConfig;

/// Persistent PG + object store for long-lived RAG smoke corpora (survives `cargo test` reruns).
pub(crate) struct PersistentSmokeInfra {
    pub postgres_url: String,
    pub object_store_path: std::path::PathBuf,
}

/// HTTP client timeout for mock RAG paths (ingestion + retrieval + synthesis).
pub(crate) const HTTP_TIMEOUT_RAG_SECS: u64 = 120;
/// HTTP client timeout when `use_real_llm` is enabled (nightly / llm_real).
pub(crate) const HTTP_TIMEOUT_REAL_LLM_SECS: u64 = 180;
/// HTTP client timeout for non-RAG smoke paths.
pub(crate) const HTTP_TIMEOUT_DEFAULT_SECS: u64 = 60;

fn http_client_timeout_secs(use_real_llm: bool, enable_rag: bool) -> u64 {
    if use_real_llm {
        HTTP_TIMEOUT_REAL_LLM_SECS
    } else if enable_rag {
        HTTP_TIMEOUT_RAG_SECS
    } else {
        HTTP_TIMEOUT_DEFAULT_SECS
    }
}

fn smoke_worker_id() -> String {
    let short_uuid = Uuid::new_v4().simple().to_string();
    format!("e2e-smoke-v5-{}", &short_uuid[..8])
}

/// Cross-process registry: one marker per migrated database URL (see setup.rs container registry).
const PG_MIGRATED_DIR: &str = "/tmp/avrag-e2e-pg-migrated";

fn pg_migrated_dir() -> std::path::PathBuf {
    std::env::var("AVRAG_E2E_PG_MIGRATED_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from(PG_MIGRATED_DIR))
}

fn ensure_pg_migrated_dir() {
    if let Err(error) = std::fs::create_dir_all(pg_migrated_dir()) {
        eprintln!("[product_e2e] pg migrated dir failed: {error}");
    }
}

fn pg_url_hash(database_url: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    database_url.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn pg_migration_marker_path(database_url: &str) -> std::path::PathBuf {
    pg_migrated_dir().join(pg_url_hash(database_url))
}

fn pg_migration_lock_path(database_url: &str) -> std::path::PathBuf {
    pg_migrated_dir().join(format!("{}.lock", pg_url_hash(database_url)))
}

fn pg_migration_wait_timeout() -> Duration {
    std::env::var("AVRAG_E2E_PG_MIGRATION_WAIT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(120))
}

fn pg_url_wait_for_migrated(database_url: &str, timeout: Duration) {
    let marker = pg_migration_marker_path(database_url);
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if marker.is_file() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    eprintln!(
        "[product_e2e] WARN: timed out waiting for pg migration marker for {}",
        pg_url_hash(database_url)
    );
}

/// Returns true when this caller should run bootstrap migrations for `database_url`.
pub(crate) fn pg_url_needs_migration(database_url: &str) -> bool {
    ensure_pg_migrated_dir();
    if pg_migration_marker_path(database_url).is_file() {
        return false;
    }
    let lock_path = pg_migration_lock_path(database_url);
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
    {
        Ok(mut file) => {
            let _ = writeln!(file, "{}", std::process::id());
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            pg_url_wait_for_migrated(database_url, pg_migration_wait_timeout());
            false
        }
        Err(error) => {
            eprintln!("[product_e2e] pg migration lock claim failed: {error}");
            false
        }
    }
}

pub(crate) fn pg_url_mark_migrated(database_url: &str) {
    ensure_pg_migrated_dir();
    let marker = pg_migration_marker_path(database_url);
    let _ = std::fs::write(&marker, std::process::id().to_string());
    let _ = std::fs::remove_file(pg_migration_lock_path(database_url));
}

pub(crate) fn pg_url_release_migration_claim(database_url: &str) {
    let _ = std::fs::remove_file(pg_migration_lock_path(database_url));
}

async fn wait_for_worker_health_port_file(
    port_file: &std::path::Path,
    timeout: Duration,
) -> anyhow::Result<u16> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Ok(content) = tokio::fs::read_to_string(port_file).await {
            if let Ok(port) = content.trim().parse::<u16>() {
                if port > 0 {
                    return Ok(port);
                }
            }
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!("worker health port file not ready: {}", port_file.display());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

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
            // PR smoke always uses mock Brave — real search belongs in llm_real / search_real.
            return false;
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
        persistent_infra: Option<&PersistentSmokeInfra>,
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
        let (org_id, user_id) = identity
            .as_ref()
            .expect("identity is always Some after .or_else above");
        let milvus_collection_prefix =
            enable_rag.then(|| milvus_collection_prefix_for_identity(org_id));

        let (pg_url, shared_pg) = if let Some(infra) = persistent_infra {
            setup::acquire_external_postgres(&infra.postgres_url)
                .await
                .expect("acquire external postgres for persistent smoke corpus")
        } else {
            setup::acquire_shared_postgres()
                .await
                .expect("start shared postgres")
        };

        let (milvus_url, shared_milvus) = if enable_rag {
            let (url, shared) = setup::acquire_shared_milvus()
                .await
                .expect("start shared milvus");
            (Some(url), Some(shared))
        } else {
            (None, None)
        };

        let object_store_dir = setup::create_temp_object_store();
        let object_root = if let Some(infra) = persistent_infra {
            std::fs::create_dir_all(&infra.object_store_path)
                .expect("create persistent object store");
            infra.object_store_path.to_string_lossy().into_owned()
        } else {
            object_store_dir.path().to_string_lossy().to_string()
        };

        let (mock_llm_url, mock_llm_abort) = if use_real_llm {
            (String::new(), None)
        } else {
            // Mock RAG dense_search query injection: only LLM request message parsing is
            // end-to-end reliable (see mock_servers::dense_search_query_from_messages).
            // Per-request chat headers and the global set_mock_rag_codegen_query cell are
            // best-effort fallbacks for single-flight tests, not concurrent paths.
            reset_mock_rag_state();
            let (url, abort) = start_mock_llm_server().await;
            (url, Some(abort))
        };

        let (mock_search_url, mock_search_abort, search_controls) =
            start_mock_search_server().await;

        let (mock_paddle_url, mock_paddle_abort, mock_paddle_jobs_submitted) = if use_real_llm {
            (String::new(), None, None)
        } else {
            let (url, abort, jobs) = start_mock_paddle_ocr_server().await;
            (url, Some(abort), Some(jobs))
        };

        let (mock_office_url, mock_office_abort) = if use_real_llm {
            (String::new(), None)
        } else {
            let (url, abort) = start_mock_office_parser_server().await;
            (url, Some(abort))
        };

        let has_real_search = Self::resolve_use_real_search(use_real_llm).await;

        let (mock_embedding_url, mock_embedding_abort, embedding_should_503, embedding_call_count) =
            if enable_rag && !use_real_llm {
                let (url, abort, flag, call_count) = start_mock_embedding_server().await;
                (Some(url), Some(abort), Some(flag), Some(call_count))
            } else {
                (None, None, None, None)
            };

        // Transport middleware still reads this single flag from env (known seam).
        unsafe {
            std::env::set_var("E2E_ENABLED", "true");
        }

        let run_migrations = pg_url_needs_migration(&pg_url);
        let redis = redis_url
            .clone()
            .unwrap_or_else(|| "redis://127.0.0.1:1".to_string());
        let worker_health_port_file = object_store_dir
            .path()
            .join("worker-health.port")
            .to_string_lossy()
            .into_owned();
        let ingestion_queue_group = if persistent_infra.is_some() {
            unsafe {
                std::env::set_var("AVRAG_INGESTION_QUEUE_GROUP", "e2e-smoke");
            }
            "e2e-smoke".to_string()
        } else {
            "default".to_string()
        };
        let bootstrap = E2eBootstrapConfig {
            org_id: org_id.clone(),
            user_id: user_id.clone(),
            database_url: pg_url.clone(),
            auto_migrate: run_migrations,
            object_root: object_root.clone(),
            enable_rag,
            redis_url: redis,
            milvus_url: milvus_url.clone(),
            milvus_collection_prefix: milvus_collection_prefix.clone(),
            mock_llm_base_url: if use_real_llm {
                None
            } else {
                Some(mock_llm_url.clone())
            },
            mock_embedding_base_url: mock_embedding_url.clone(),
            mock_search_base_url: if has_real_search {
                None
            } else {
                Some(mock_search_url.clone())
            },
            mock_paddle_ocr_base_url: if use_real_llm {
                None
            } else {
                Some(mock_paddle_url.clone())
            },
            mock_office_parser_base_url: if use_real_llm {
                None
            } else {
                Some(mock_office_url.clone())
            },
            use_real_llm,
            has_real_search,
            worker_timeout_secs,
            worker_health_port_file,
            ingestion_queue_group,
        };

        let (listener, base_url) = bind_persistent_listener().await;

        let config = bootstrap.build_app_config(&base_url);
        let state = match app::AppState::bootstrap(config.clone()).await {
            Ok(state) => state,
            Err(error) => {
                if run_migrations {
                    pg_url_release_migration_claim(&pg_url);
                }
                panic!("bootstrap AppState: {error}");
            }
        };
        if run_migrations {
            pg_url_mark_migrated(&pg_url);
        }
        let app_state = Arc::new(state.clone());

        let router = app::product_e2e_http::build_router(state);

        let (abort_tx, abort_rx) = tokio::sync::oneshot::channel::<()>();
        spawn_persistent(async move {
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
        let mut worker_bootstrap = bootstrap.clone();
        worker_bootstrap.auto_migrate = false;
        let worker_id = smoke_worker_id();
        worker_bootstrap.apply_worker_env(&mut cmd, &base_url, Some(&worker_id));
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

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

        let worker_health_port = wait_for_worker_health_port_file(
            std::path::Path::new(&worker_bootstrap.worker_health_port_file),
            Duration::from_secs(10),
        )
        .await
        .expect("worker health port file ready");
        wait_for_worker_health(worker_health_port, Duration::from_secs(10))
            .await
            .expect("worker health ready");

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(http_client_timeout_secs(
                use_real_llm,
                enable_rag,
            )))
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
            org_id: org_id.clone(),
            user_id: user_id.clone(),
            app_state: Some(app_state),
            bootstrap: Some(bootstrap),
            shared_pg: Some(shared_pg),
            shared_milvus,
            milvus_url,
            milvus_collection_prefix,
            worker: Some(worker),
            server_abort: Some(abort_tx),
            object_store_dir,
            object_root,
            pg_url,
            mock_llm_abort,
            mock_embedding_abort,
            mock_search_abort: Some(mock_search_abort),
            mock_paddle_abort,
            mock_paddle_jobs_submitted,
            mock_office_abort,
            search_controls: Some(search_controls),
            embedding_should_503,
            embedding_call_count,
            redis_container_name,
            worker_log_path: Some(worker_log_path),
            artifact_run_id,
        }
    }

    /// Attach a fresh worker to the module-scoped smoke-v5 corpus infra (real LLM timeouts).
    pub(crate) async fn spawn_from_smoke_v5_fixture(
        fixture: &super::super::fixtures::SmokeV5CorpusFixture,
    ) -> Self {
        reset_mock_rag_state();

        let base_url = fixture.api_base_url.clone();
        let placeholder_dir = tempfile::tempdir().expect("placeholder object store");
        let worker_health_port_file = placeholder_dir
            .path()
            .join(format!("worker-health-{}.port", Uuid::new_v4().simple()))
            .to_string_lossy()
            .into_owned();

        let mut bootstrap = fixture.worker_bootstrap.clone();
        bootstrap.auto_migrate = false;
        bootstrap.worker_health_port_file = worker_health_port_file;

        let worker_binary = setup::find_worker_binary()
            .await
            .expect("find worker binary");
        let worker_log_path = placeholder_dir.path().join("worker.log");
        let mut cmd = tokio::process::Command::new(&worker_binary);
        let mut worker_bootstrap = bootstrap.clone();
        worker_bootstrap.auto_migrate = false;
        let worker_id = smoke_worker_id();
        worker_bootstrap.apply_worker_env(&mut cmd, &base_url, Some(&worker_id));
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

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

        let worker_health_port = wait_for_worker_health_port_file(
            std::path::Path::new(&worker_bootstrap.worker_health_port_file),
            Duration::from_secs(10),
        )
        .await
        .expect("worker health port file ready");
        wait_for_worker_health(worker_health_port, Duration::from_secs(10))
            .await
            .expect("worker health ready");

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(HTTP_TIMEOUT_REAL_LLM_SECS))
            .default_headers(test_auth_headers_for(&fixture.org_id, &fixture.user_id))
            .build()
            .expect("reqwest client build");

        let now = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let short_commit = option_env!("GITHUB_SHA")
            .map(|s| &s[..s.len().min(8)])
            .unwrap_or("local");
        let artifact_run_id = format!("e2e_{now}_{short_commit}_{}", Uuid::new_v4().simple());
        let object_root = bootstrap.object_root.clone();

        Self {
            http_client: client,
            base_url,
            org_id: fixture.org_id.clone(),
            user_id: fixture.user_id.clone(),
            app_state: Some(fixture.app_state.clone()),
            bootstrap: Some(bootstrap),
            shared_pg: None,
            shared_milvus: None,
            milvus_url: Some(fixture.milvus_url.clone()),
            milvus_collection_prefix: None,
            worker: Some(worker),
            server_abort: None,
            object_store_dir: placeholder_dir,
            object_root,
            pg_url: fixture.pg_url.clone(),
            mock_llm_abort: None,
            mock_embedding_abort: None,
            mock_search_abort: None,
            mock_paddle_abort: None,
            mock_paddle_jobs_submitted: None,
            mock_office_abort: None,
            search_controls: None,
            embedding_should_503: None,
            embedding_call_count: None,
            redis_container_name: None,
            worker_log_path: Some(worker_log_path),
            artifact_run_id,
        }
    }

    /// Attach a fresh worker to the module-scoped API + shared RAG infra.
    pub(crate) async fn spawn_from_rag_fixture(
        fixture: &super::super::fixtures::RagSharedFixture,
    ) -> Self {
        reset_mock_rag_state();

        let base_url = fixture.api_base_url.clone();
        let placeholder_dir = tempfile::tempdir().expect("placeholder object store");
        let worker_health_port_file = placeholder_dir
            .path()
            .join(format!("worker-health-{}.port", Uuid::new_v4().simple()))
            .to_string_lossy()
            .into_owned();

        let mut bootstrap = fixture.worker_bootstrap.clone();
        bootstrap.auto_migrate = false;
        bootstrap.worker_health_port_file = worker_health_port_file;

        let worker_binary = setup::find_worker_binary()
            .await
            .expect("find worker binary");
        let worker_log_path = placeholder_dir.path().join("worker.log");
        let mut cmd = tokio::process::Command::new(&worker_binary);
        let mut worker_bootstrap = bootstrap.clone();
        worker_bootstrap.auto_migrate = false;
        let worker_id = smoke_worker_id();
        worker_bootstrap.apply_worker_env(&mut cmd, &base_url, Some(&worker_id));
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

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

        let worker_health_port = wait_for_worker_health_port_file(
            std::path::Path::new(&worker_bootstrap.worker_health_port_file),
            Duration::from_secs(10),
        )
        .await
        .expect("worker health port file ready");
        wait_for_worker_health(worker_health_port, Duration::from_secs(10))
            .await
            .expect("worker health ready");

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(HTTP_TIMEOUT_RAG_SECS))
            .default_headers(test_auth_headers_for(&fixture.org_id, &fixture.user_id))
            .build()
            .expect("reqwest client build");

        let now = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let short_commit = option_env!("GITHUB_SHA")
            .map(|s| &s[..s.len().min(8)])
            .unwrap_or("local");
        let artifact_run_id = format!("e2e_{now}_{short_commit}_{}", Uuid::new_v4().simple());
        let object_root = bootstrap.object_root.clone();

        Self {
            http_client: client,
            base_url,
            org_id: fixture.org_id.clone(),
            user_id: fixture.user_id.clone(),
            app_state: Some(fixture.app_state.clone()),
            bootstrap: Some(bootstrap),
            // Infra ref-counting is owned by the module-scoped [`RagSharedFixture`].
            shared_pg: None,
            shared_milvus: None,
            milvus_url: Some(fixture.milvus_url.clone()),
            // Collection cleanup is owned by the module-scoped [`RagSharedFixture`].
            milvus_collection_prefix: None,
            worker: Some(worker),
            server_abort: None,
            object_store_dir: placeholder_dir,
            object_root,
            pg_url: fixture.pg_url.clone(),
            mock_llm_abort: None,
            mock_embedding_abort: None,
            mock_search_abort: None,
            mock_paddle_abort: None,
            mock_paddle_jobs_submitted: None,
            mock_office_abort: None,
            search_controls: fixture.search_controls.clone(),
            embedding_should_503: fixture.embedding_should_503.clone(),
            embedding_call_count: fixture.embedding_call_count.clone(),
            redis_container_name: None,
            worker_log_path: Some(worker_log_path),
            artifact_run_id,
        }
    }
}
