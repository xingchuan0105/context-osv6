//! Test infrastructure setup — Docker-based Postgres + Milvus + ephemeral HTTP server + worker spawn.
//!
//! Design: minimal external dependencies (no testcontainers crate), uses docker CLI directly.

use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Cross-process registry of containers currently owned by a running E2E test.
const ACTIVE_CONTAINER_DIR: &str = "/tmp/avrag-e2e-active-containers";

/// Only remove unmatched containers older than this (parallel runs stay safe).
const ORPHAN_MIN_AGE_SECS: u64 = 120;

/// Registry markers outlive a crashed container; prune when PID is dead and container is gone or ancient.
const STALE_REGISTRY_AGE_SECS: u64 = 600;

fn ensure_active_container_dir() -> std::io::Result<()> {
    std::fs::create_dir_all(ACTIVE_CONTAINER_DIR)
}

/// Mark a test-owned container as in-use so orphan cleanup will not delete it.
pub fn register_active_test_container(container_name: &str) -> bool {
    if let Err(error) = ensure_active_container_dir() {
        eprintln!("[product_e2e] register container dir failed: {error}");
        return false;
    }
    let path = format!("{ACTIVE_CONTAINER_DIR}/{container_name}");
    match std::fs::write(path, std::process::id().to_string()) {
        Ok(()) => true,
        Err(error) => {
            eprintln!(
                "[product_e2e] failed to register active container {container_name}: {error}"
            );
            false
        }
    }
}

/// Clear the in-use marker when a test releases its container.
pub fn unregister_active_test_container(container_name: &str) {
    let path = format!("{ACTIVE_CONTAINER_DIR}/{container_name}");
    let _ = std::fs::remove_file(path);
}

fn is_active_test_container(container_name: &str) -> bool {
    std::path::Path::new(ACTIVE_CONTAINER_DIR)
        .join(container_name)
        .is_file()
}

async fn docker_container_exists(container_name: &str) -> bool {
    tokio::process::Command::new("docker")
        .args(["inspect", container_name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|status| status.success())
        .unwrap_or(false)
}

fn parse_docker_inspect_timestamp(raw: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.starts_with("0001-01-01") {
        return None;
    }
    chrono::DateTime::parse_from_rfc3339(trimmed)
        .ok()
        .or_else(|| {
            chrono::DateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S.%f %z %Z").ok()
        })
        .or_else(|| chrono::DateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S %z %Z").ok())
}

async fn docker_container_age_secs(container_name: &str) -> Option<u64> {
    let output = tokio::process::Command::new("docker")
        .args([
            "inspect",
            "--format",
            "{{.CreatedAt}}",
            container_name,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let created_raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let created = parse_docker_inspect_timestamp(&created_raw)?;
    let created_ms = created.timestamp_millis().max(0) as u64;
    if created_ms == 0 {
        return None;
    }
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    Some(now_ms.saturating_sub(created_ms) / 1000)
}

fn is_pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    std::path::Path::new(&format!("/proc/{pid}")).exists()
}

async fn prune_stale_active_registry() {
    let Ok(entries) = std::fs::read_dir(ACTIVE_CONTAINER_DIR) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_name) = entry.file_name().into_string() else {
            continue;
        };
        let pid = std::fs::read_to_string(entry.path())
            .ok()
            .and_then(|raw| raw.trim().parse::<u32>().ok())
            .unwrap_or(0);
        if is_pid_alive(pid) {
            continue;
        }
        let container_exists = docker_container_exists(&file_name).await;
        let should_remove = if !container_exists {
            true
        } else {
            match docker_container_age_secs(&file_name).await {
                Some(age_secs) => age_secs >= STALE_REGISTRY_AGE_SECS,
                None => false,
            }
        };
        if should_remove {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

async fn is_safe_to_remove_orphan(container_name: &str) -> bool {
    if is_active_test_container(container_name) {
        return false;
    }
    match docker_container_age_secs(container_name).await {
        Some(age_secs) => age_secs >= ORPHAN_MIN_AGE_SECS,
        // Unknown age: keep the container rather than risk killing a parallel test.
        None => false,
    }
}

// ---------------------------------------------------------------------------
// Postgres
// ---------------------------------------------------------------------------

/// Process-wide shared Postgres for product E2E tests.
///
/// One container per test binary; reference-counted so only the last
/// [`TestContext`] drop stops it.
pub struct SharedPostgres {
    pub url: String,
    pub container_name: String,
    refs: AtomicUsize,
}

static SHARED_PG: OnceLock<tokio::sync::Mutex<Option<Arc<SharedPostgres>>>> = OnceLock::new();

fn shared_pg_slot() -> &'static tokio::sync::Mutex<Option<Arc<SharedPostgres>>> {
    SHARED_PG.get_or_init(|| tokio::sync::Mutex::new(None))
}

fn short_docker_id() -> String {
    Uuid::new_v4().to_string()
}

async fn postgres_is_ready(url: &str) -> bool {
    match avrag_storage_pg::PgAppRepository::connect(url).await {
        Ok(repo) => repo.ping().await.is_ok(),
        Err(_) => false,
    }
}

/// Acquire a reference to the shared Postgres container, creating it on first use.
pub async fn acquire_shared_postgres() -> anyhow::Result<(String, Arc<SharedPostgres>)> {
    let existing = {
        let slot = shared_pg_slot().lock().await;
        slot.clone()
    };

    if let Some(pg) = existing {
        if postgres_is_ready(&pg.url).await {
            pg.refs.fetch_add(1, Ordering::SeqCst);
            return Ok((pg.url.clone(), pg));
        }
        let stale_name = pg.container_name.clone();
        let _ = stop_postgres(&stale_name).await;
        let mut slot = shared_pg_slot().lock().await;
        if slot.as_ref().is_some_and(|shared| Arc::ptr_eq(shared, &pg)) {
            *slot = None;
        }
    }

    let (url, container_name) = start_postgres().await?;
    let pg = Arc::new(SharedPostgres {
        url: url.clone(),
        container_name,
        refs: AtomicUsize::new(1),
    });
    let mut slot = shared_pg_slot().lock().await;
    *slot = Some(pg.clone());
    Ok((url, pg))
}

/// Release a shared Postgres reference; stops the container when the last ref drops.
pub fn release_shared_postgres(pg: &Arc<SharedPostgres>) {
    let prev = pg.refs.fetch_sub(1, Ordering::SeqCst);
    if prev == 1 {
        let container_name = pg.container_name.clone();
        block_on_with_timeout(async move {
            stop_postgres(&container_name).await;
        });
        let mut slot = shared_pg_slot().blocking_lock();
        if slot.as_ref().is_some_and(|shared| Arc::ptr_eq(shared, pg)) {
            *slot = None;
        }
    }
}

/// Start a Postgres container via docker and return `(url, container_name)`.
///
/// Retries up to 3 times with a fresh ephemeral port on bind/forward failures.
pub async fn start_postgres() -> anyhow::Result<(String, String)> {
    const MAX_ATTEMPTS: u32 = 3;
    let mut last_err = None;

    for attempt in 1..=MAX_ATTEMPTS {
        match start_postgres_once().await {
            Ok(pair) => return Ok(pair),
            Err(e) => {
                eprintln!(
                    "[product_e2e] start_postgres attempt {attempt}/{MAX_ATTEMPTS} failed: {e}"
                );
                last_err = Some(e);
                if attempt < MAX_ATTEMPTS {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("start_postgres failed")))
}

async fn start_postgres_once() -> anyhow::Result<(String, String)> {
    let container_name = format!("avrag-test-pg-{}", short_docker_id());

    let output = tokio::process::Command::new("docker")
        .args([
            "run",
            "-d",
            "--rm",
            "--name",
            &container_name,
            "-e",
            "POSTGRES_USER=test",
            "-e",
            "POSTGRES_PASSWORD=test",
            "-e",
            "POSTGRES_DB=test",
            "-p",
            "0:5432",
            "postgres:16-alpine",
        ])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = tokio::process::Command::new("docker")
            .args(["rm", "-f", &container_name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
        anyhow::bail!("docker run postgres failed: {stderr}");
    }
    register_active_test_container(&container_name);

    let port = match docker_mapped_port(&container_name, 5432).await {
        Ok(port) => port,
        Err(e) => {
            unregister_active_test_container(&container_name);
            let _ = tokio::process::Command::new("docker")
                .args(["rm", "-f", &container_name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;
            return Err(e);
        }
    };
    let url = format!("postgres://test:test@127.0.0.1:{port}/test");
    if let Err(error) = wait_for_postgres(&url, &container_name).await {
        unregister_active_test_container(&container_name);
        return Err(error);
    }
    Ok((url, container_name))
}

fn block_on_with_timeout(fut: impl std::future::Future<Output = ()> + Send + 'static) {
    std::thread::Builder::new()
        .name("product-e2e-teardown".into())
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime for sync teardown")
                .block_on(async {
                    let _ = tokio::time::timeout(Duration::from_secs(10), fut).await;
                });
        })
        .expect("spawn teardown thread")
        .join()
        .expect("join teardown thread");
}

/// Synchronously stop a Milvus container (10s timeout) for test teardown.
pub fn sync_stop_milvus(container_name: &str) {
    let name = container_name.to_string();
    block_on_with_timeout(async move {
        stop_milvus(&name).await;
    });
}

/// Synchronously drop Milvus collections (10s timeout) for test teardown.
pub fn sync_drop_milvus_collections(prefix: &str) {
    let prefix = prefix.to_string();
    block_on_with_timeout(async move {
        drop_milvus_collections(&prefix).await;
    });
}

/// Stop a Postgres container by name.
pub async fn stop_postgres(container_name: &str) {
    unregister_active_test_container(container_name);
    let _ = tokio::process::Command::new("docker")
        .args(["stop", "-t", "3", container_name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
}

async fn wait_for_postgres(url: &str, container_name: &str) -> anyhow::Result<()> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        match avrag_storage_pg::PgAppRepository::connect(url).await {
            Ok(repo) => {
                if repo.ping().await.is_ok() {
                    return Ok(());
                }
            }
            Err(_) if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            Err(e) => {
                let _ = stop_postgres(container_name).await;
                anyhow::bail!("postgres did not become ready in 30s: {e}");
            }
        }
    }
}

#[allow(dead_code)]
fn find_ephemeral_port() -> anyhow::Result<u16> {
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

fn parse_docker_port_output(output: &str) -> anyhow::Result<u16> {
    for line in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if let Some(port) = line.rsplit(':').next().and_then(|part| part.parse().ok()) {
            return Ok(port);
        }
    }
    anyhow::bail!("missing docker port mapping in output: {output:?}")
}

async fn docker_mapped_port(container_name: &str, container_port: u16) -> anyhow::Result<u16> {
    let output = tokio::process::Command::new("docker")
        .args(["port", container_name, &format!("{container_port}/tcp")])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("docker port {container_name} {container_port}/tcp failed: {stderr}");
    }

    parse_docker_port_output(&String::from_utf8_lossy(&output.stdout))
}

/// Create a temporary object store directory.
pub fn create_temp_object_store() -> tempfile::TempDir {
    tempfile::tempdir().expect("create tempdir")
}

/// Infer MIME type from a filename extension (E2E uploads only).
pub fn mime_type_for_filename(filename: &str) -> &'static str {
    let lower = filename.to_ascii_lowercase();
    if lower.ends_with(".pdf") {
        "application/pdf"
    } else if lower.ends_with(".txt") {
        "text/plain"
    } else if lower.ends_with(".md") {
        "text/markdown"
    } else {
        "application/octet-stream"
    }
}

/// Load fixture content from `tests/product_e2e/fixtures/`.
pub fn load_fixture(name: &str) -> anyhow::Result<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/product_e2e/fixtures")
        .join(name);
    std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("failed to read fixture {}: {}", path.display(), e))
}

// ---------------------------------------------------------------------------
// Milvus (standalone)
// ---------------------------------------------------------------------------

/// Handle to a running Milvus instance.
///
/// `is_external == true` means we did not start the container and must not
/// stop it in `Drop` (it may be a developer's local instance or a leftover
/// from a prior test run that we cannot safely kill).
pub struct MilvusInstance {
    pub url: String,
    pub container_name: Option<String>,
    pub is_external: bool,
}

/// Process-wide shared Milvus for RAG product E2E tests.
pub struct SharedMilvus {
    pub url: String,
    pub container_name: Option<String>,
    pub is_external: bool,
    refs: AtomicUsize,
}

static SHARED_MILVUS: OnceLock<tokio::sync::Mutex<Option<Arc<SharedMilvus>>>> = OnceLock::new();

fn shared_milvus_slot() -> &'static tokio::sync::Mutex<Option<Arc<SharedMilvus>>> {
    SHARED_MILVUS.get_or_init(|| tokio::sync::Mutex::new(None))
}

/// Acquire a shared Milvus instance, creating it on first use.
pub async fn acquire_shared_milvus() -> anyhow::Result<(String, Arc<SharedMilvus>)> {
    let existing = {
        let slot = shared_milvus_slot().lock().await;
        slot.clone()
    };

    if let Some(milvus) = existing {
        if milvus_api_ready(&milvus.url).await {
            milvus.refs.fetch_add(1, Ordering::SeqCst);
            return Ok((milvus.url.clone(), milvus));
        }
        if !milvus.is_external
            && let Some(ref stale_name) = milvus.container_name
        {
            let _ = stop_milvus(stale_name).await;
        }
        let mut slot = shared_milvus_slot().lock().await;
        if slot.as_ref().is_some_and(|shared| Arc::ptr_eq(shared, &milvus)) {
            *slot = None;
        }
    }

    let inst = start_milvus().await?;
    let milvus = Arc::new(SharedMilvus {
        url: inst.url.clone(),
        container_name: inst.container_name,
        is_external: inst.is_external,
        refs: AtomicUsize::new(1),
    });
    let mut slot = shared_milvus_slot().lock().await;
    *slot = Some(milvus.clone());
    Ok((inst.url, milvus))
}

/// Release a shared Milvus reference; stops only test-owned containers.
pub fn release_shared_milvus(milvus: &Arc<SharedMilvus>) {
    let prev = milvus.refs.fetch_sub(1, Ordering::SeqCst);
    if prev == 1 {
        if !milvus.is_external
            && let Some(ref container_name) = milvus.container_name
        {
            let container_name = container_name.clone();
            block_on_with_timeout(async move {
                stop_milvus(&container_name).await;
            });
        }
        let mut slot = shared_milvus_slot().blocking_lock();
        if slot.as_ref().is_some_and(|shared| Arc::ptr_eq(shared, milvus)) {
            *slot = None;
        }
    }
}

fn milvus_compose_file() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docker-compose.milvus.yml")
}

const MILVUS_STANDALONE_CONTAINER: &str = "milvus-standalone";

/// Start or reuse a Milvus instance for product E2E.
///
/// Prefers a healthy external instance on 19530 (fast, already warmed).
/// Falls back to the project compose stack (etcd + minio + standalone) when
/// 19530 is unreachable. A pre-existing healthy instance on 19530 is treated as
/// external (developer stack; no register/stop). A compose stack started by this
/// test run is test-owned (`is_external: false`) and registered for orphan safety.
/// Collection isolation relies on per-context `MILVUS_COLLECTION_PREFIX` +
/// teardown drops, not on dedicated Milvus processes.
pub async fn start_milvus() -> anyhow::Result<MilvusInstance> {
    let external_url = "http://127.0.0.1:19530";
    if milvus_api_ready(external_url).await {
        return Ok(MilvusInstance {
            url: external_url.to_string(),
            container_name: Some(MILVUS_STANDALONE_CONTAINER.to_string()),
            is_external: true,
        });
    }

    start_milvus_compose_stack().await?;
    wait_for_milvus(external_url, MILVUS_STANDALONE_CONTAINER).await?;
    register_active_test_container(MILVUS_STANDALONE_CONTAINER);
    Ok(MilvusInstance {
        url: external_url.to_string(),
        container_name: Some(MILVUS_STANDALONE_CONTAINER.to_string()),
        is_external: false,
    })
}

async fn start_milvus_compose_stack() -> anyhow::Result<()> {
    let compose_path = milvus_compose_file();
    if !compose_path.is_file() {
        anyhow::bail!(
            "Milvus not reachable on 127.0.0.1:19530 and compose file missing at {}",
            compose_path.display()
        );
    }
    let compose_dir = compose_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("compose file has no parent: {}", compose_path.display()))?;

    let output = tokio::process::Command::new("docker")
        .args([
            "compose",
            "-f",
            compose_path.to_str().expect("compose path utf-8"),
            "up",
            "-d",
        ])
        .current_dir(compose_dir)
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!(
            "docker compose milvus up failed (stderr={stderr}, stdout={stdout})"
        );
    }
    Ok(())
}

/// Stop a Milvus container by name.
pub async fn stop_milvus(container_name: &str) {
    let _ = tokio::process::Command::new("docker")
        .args(["stop", "-t", "3", container_name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
    if is_active_test_container(container_name) {
        unregister_active_test_container(container_name);
    }
}

/// Drop all collections belonging to a given prefix via the Milvus REST API.
///
/// This prevents test vectors from accumulating across runs and polluting
/// similarity-search results for subsequent tests.
pub async fn drop_milvus_collections(prefix: &str) {
    let milvus_url =
        std::env::var("MILVUS_URL").unwrap_or_else(|_| "http://127.0.0.1:19530".to_string());
    let client = reqwest::Client::new();
    let collections = [
        format!("{prefix}_rag_text_chunks"),
        format!("{prefix}_rag_multimodal_chunks"),
        format!("{prefix}_rag_kg_entities"),
        format!("{prefix}_rag_kg_relations"),
        format!("{prefix}_rag_graph_passages"),
    ];
    for name in &collections {
        let body = serde_json::json!({
            "dbName": std::env::var("MILVUS_DATABASE").unwrap_or_else(|_| "default".to_string()),
            "collectionName": name,
        });
        let res = client
            .post(format!("{milvus_url}/v2/vectordb/collections/drop"))
            .json(&body)
            .send()
            .await;
        match res {
            Ok(r) => {
                let status = r.status();
                if status.is_success() {
                    eprintln!("[product_e2e] dropped Milvus collection: {name}");
                } else {
                    let text = r.text().await.unwrap_or_default();
                    // 400 = collection not found is fine (already clean)
                    if status.as_u16() != 400 || !text.contains("not found") {
                        eprintln!(
                            "[product_e2e] drop collection {name} returned HTTP {status}: {text}"
                        );
                    }
                }
            }
            Err(e) => {
                eprintln!("[product_e2e] failed to drop collection {name}: {e}");
            }
        }
    }
}

async fn milvus_api_ready(url: &str) -> bool {
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    else {
        return false;
    };
    let body = serde_json::json!({ "dbName": "default" });
    let Ok(res) = client
        .post(format!("{url}/v2/vectordb/collections/list"))
        .json(&body)
        .send()
        .await
    else {
        return false;
    };
    res.status().is_success()
}

async fn docker_container_running(container_name: &str) -> Option<bool> {
    let output = tokio::process::Command::new("docker")
        .args(["inspect", "-f", "{{.State.Running}}", container_name])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    match String::from_utf8_lossy(&output.stdout).trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

async fn docker_container_recent_logs(container_name: &str) -> String {
    let output = tokio::process::Command::new("docker")
        .args(["logs", "--tail", "40", container_name])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            format!("{stdout}{stderr}")
        }
        Err(e) => format!("(failed to read docker logs: {e})"),
    }
}

async fn wait_for_milvus(url: &str, container_name: &str) -> anyhow::Result<()> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
    loop {
        if milvus_api_ready(url).await {
            return Ok(());
        }
        if let Some(false) = docker_container_running(container_name).await {
            let logs = docker_container_recent_logs(container_name).await;
            anyhow::bail!(
                "milvus container {container_name} exited before becoming ready at {url}; recent logs:\n{logs}"
            );
        }
        if tokio::time::Instant::now() >= deadline {
            let logs = docker_container_recent_logs(container_name).await;
            anyhow::bail!(
                "milvus did not become ready in 90s at {url}; recent logs:\n{logs}"
            );
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

// ---------------------------------------------------------------------------
// Redis (embedding cache tests)
// ---------------------------------------------------------------------------

/// Start a Redis container via docker and return `(url, container_name)`.
pub async fn start_redis() -> anyhow::Result<(String, String)> {
    let container_name = format!("avrag-test-redis-{}", short_docker_id());

    let output = tokio::process::Command::new("docker")
        .args([
            "run",
            "-d",
            "--rm",
            "--name",
            &container_name,
            "-p",
            "0:6379",
            "redis:7-alpine",
        ])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("docker run redis failed: {stderr}");
    }
    register_active_test_container(&container_name);

    let port = match docker_mapped_port(&container_name, 6379).await {
        Ok(port) => port,
        Err(e) => {
            unregister_active_test_container(&container_name);
            let _ = tokio::process::Command::new("docker")
                .args(["rm", "-f", &container_name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;
            return Err(e);
        }
    };
    let url = format!("redis://127.0.0.1:{port}");
    if let Err(error) = wait_for_redis(&url, &container_name).await {
        unregister_active_test_container(&container_name);
        return Err(error);
    }
    Ok((url, container_name))
}

/// Synchronously stop a Redis container (10s timeout) for test teardown.
pub fn sync_stop_redis(container_name: &str) {
    let name = container_name.to_string();
    block_on_with_timeout(async move {
        stop_redis(&name).await;
    });
}

/// Stop a Redis container by name.
pub async fn stop_redis(container_name: &str) {
    unregister_active_test_container(container_name);
    let _ = tokio::process::Command::new("docker")
        .args(["stop", "-t", "3", container_name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
}

async fn wait_for_redis(url: &str, container_name: &str) -> anyhow::Result<()> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        if redis_ping(url).await {
            return Ok(());
        }
        if tokio::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(500)).await;
        } else {
            let _ = stop_redis(container_name).await;
            anyhow::bail!("redis did not become ready in 30s");
        }
    }
}

pub async fn redis_ping(url: &str) -> bool {
    let client = match redis::Client::open(url) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let Ok(mut conn) = client.get_multiplexed_async_connection().await else {
        return false;
    };
    redis::cmd("PING")
        .query_async::<String>(&mut conn)
        .await
        .is_ok()
}

// ---------------------------------------------------------------------------
// Worker binary
// ---------------------------------------------------------------------------

/// Find the compiled worker binary path.
///
/// Tries `target/debug/avrag-worker` first, then falls back to `cargo build -p avrag-worker`.
pub async fn find_worker_binary() -> anyhow::Result<std::path::PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let candidate = manifest_dir.join("../../../target/debug/avrag-worker");
    if candidate.exists() {
        return Ok(candidate);
    }
    let candidate2 = manifest_dir.join("../../target/debug/avrag-worker");
    if candidate2.exists() {
        return Ok(candidate2);
    }

    // Build it
    let status = tokio::process::Command::new("cargo")
        .args(["build", "-p", "avrag-worker"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await?;
    if !status.success() {
        anyhow::bail!("cargo build -p avrag-worker failed");
    }

    if candidate.exists() {
        Ok(candidate)
    } else if candidate2.exists() {
        Ok(candidate2)
    } else {
        anyhow::bail!("avrag-worker binary not found after build");
    }
}

// ---------------------------------------------------------------------------
// Orphan cleanup
// ---------------------------------------------------------------------------

/// Remove stale `avrag-test-pg-*` / `avrag-test-redis-*` containers from crashed runs.
///
/// Milvus compose uses fixed names (`milvus-standalone`); those are external unless
/// registered by a test that started compose via [`start_milvus`].
///
/// Skips containers that are:
/// - registered in [`register_active_test_container`] (another parallel E2E process), or
/// - younger than [`ORPHAN_MIN_AGE_SECS`] (race window while a test is bootstrapping).
pub async fn cleanup_orphaned_test_containers() -> anyhow::Result<usize> {
    let _ = ensure_active_container_dir();
    prune_stale_active_registry().await;

    let mut removed = 0usize;
    for prefix in ["avrag-test-pg-", "avrag-test-redis-"] {
        let output = tokio::process::Command::new("docker")
            .args([
                "ps",
                "-a",
                "--filter",
                &format!("name={prefix}"),
                "--format",
                "{{.Names}}",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .await?;
        if !output.status.success() {
            continue;
        }
        let names = String::from_utf8_lossy(&output.stdout);
        for name in names.lines().filter(|s| !s.trim().is_empty()) {
            let name = name.trim();
            if !is_safe_to_remove_orphan(name).await {
                continue;
            }
            if is_active_test_container(name) {
                continue;
            }
            let status = tokio::process::Command::new("docker")
                .args(["rm", "-f", name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await?;
            if status.success() {
                eprintln!("[product_e2e] cleaned up stale orphan container: {name}");
                removed += 1;
            }
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_docker_port_output_uses_ipv4_mapping() {
        let output = "0.0.0.0:32771\n[::]:32771\n";

        let port = parse_docker_port_output(output).unwrap();

        assert_eq!(port, 32771);
    }

    #[test]
    fn active_container_registry_marks_in_use_containers() {
        let name = format!("avrag-test-pg-registry-{}", short_docker_id());
        assert!(!is_active_test_container(&name));
        assert!(register_active_test_container(&name));
        assert!(is_active_test_container(&name));
        unregister_active_test_container(&name);
        assert!(!is_active_test_container(&name));
    }

    #[test]
    fn parse_docker_inspect_timestamp_rejects_zero_dates() {
        assert!(parse_docker_inspect_timestamp("").is_none());
        assert!(parse_docker_inspect_timestamp("0001-01-01T00:00:00Z").is_none());
    }

    #[test]
    fn short_docker_id_is_full_uuid() {
        let id = short_docker_id();
        assert_eq!(id.len(), 36);
        assert!(Uuid::parse_str(&id).is_ok());
    }

    #[test]
    fn is_pid_alive_detects_current_process() {
        assert!(is_pid_alive(std::process::id()));
        assert!(!is_pid_alive(0));
    }

    #[test]
    fn parse_docker_port_output_rejects_missing_mapping() {
        let err = parse_docker_port_output("").unwrap_err();

        assert!(err.to_string().contains("missing docker port mapping"));
    }
}
