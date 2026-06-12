//! Test infrastructure setup — Docker-based Postgres + Milvus + ephemeral HTTP server + worker spawn.
//!
//! Design: minimal external dependencies (no testcontainers crate), uses docker CLI directly.

use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use uuid::Uuid;

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

static SHARED_PG: OnceLock<Mutex<Option<Arc<SharedPostgres>>>> = OnceLock::new();

fn shared_pg_slot() -> &'static Mutex<Option<Arc<SharedPostgres>>> {
    SHARED_PG.get_or_init(|| Mutex::new(None))
}

fn short_docker_id() -> String {
    Uuid::new_v4().to_string()[..8].to_string()
}

async fn postgres_is_ready(url: &str) -> bool {
    match avrag_storage_pg::PgAppRepository::connect(url).await {
        Ok(repo) => repo.ping().await.is_ok(),
        Err(_) => false,
    }
}

/// Acquire a reference to the shared Postgres container, creating it on first use.
pub async fn acquire_shared_postgres() -> anyhow::Result<(String, Arc<SharedPostgres>)> {
    let mut slot = shared_pg_slot()
        .lock()
        .map_err(|_| anyhow::anyhow!("shared postgres lock poisoned"))?;

    if let Some(pg) = slot.as_ref() {
        if postgres_is_ready(&pg.url).await {
            pg.refs.fetch_add(1, Ordering::SeqCst);
            return Ok((pg.url.clone(), pg.clone()));
        }
        let stale_name = pg.container_name.clone();
        let _ = stop_postgres(&stale_name).await;
        *slot = None;
    }

    let (url, container_name) = start_postgres().await?;
    let pg = Arc::new(SharedPostgres {
        url: url.clone(),
        container_name,
        refs: AtomicUsize::new(1),
    });
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
        if let Ok(mut slot) = shared_pg_slot().lock() {
            if slot.as_ref().is_some_and(|shared| Arc::ptr_eq(shared, pg)) {
                *slot = None;
            }
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

    let port = match docker_mapped_port(&container_name, 5432).await {
        Ok(port) => port,
        Err(e) => {
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
    wait_for_postgres(&url, &container_name).await?;
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

static SHARED_MILVUS: OnceLock<Mutex<Option<Arc<SharedMilvus>>>> = OnceLock::new();

fn shared_milvus_slot() -> &'static Mutex<Option<Arc<SharedMilvus>>> {
    SHARED_MILVUS.get_or_init(|| Mutex::new(None))
}

/// Acquire a shared Milvus instance, creating it on first use.
pub async fn acquire_shared_milvus() -> anyhow::Result<(String, Arc<SharedMilvus>)> {
    let mut slot = shared_milvus_slot()
        .lock()
        .map_err(|_| anyhow::anyhow!("shared milvus lock poisoned"))?;

    if let Some(milvus) = slot.as_ref() {
        if milvus_api_ready(&milvus.url).await {
            milvus.refs.fetch_add(1, Ordering::SeqCst);
            return Ok((milvus.url.clone(), milvus.clone()));
        }
        if !milvus.is_external
            && let Some(ref stale_name) = milvus.container_name
        {
            let _ = stop_milvus(stale_name).await;
        }
        *slot = None;
    }

    let inst = start_milvus().await?;
    let milvus = Arc::new(SharedMilvus {
        url: inst.url.clone(),
        container_name: inst.container_name,
        is_external: inst.is_external,
        refs: AtomicUsize::new(1),
    });
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
        if let Ok(mut slot) = shared_milvus_slot().lock() {
            if slot
                .as_ref()
                .is_some_and(|shared| Arc::ptr_eq(shared, milvus))
            {
                *slot = None;
            }
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
/// 19530 is unreachable. Compose services are treated as external so teardown
/// does not stop a developer's local stack.
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
    Ok(MilvusInstance {
        url: external_url.to_string(),
        container_name: Some(MILVUS_STANDALONE_CONTAINER.to_string()),
        is_external: true,
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

    let port = match docker_mapped_port(&container_name, 6379).await {
        Ok(port) => port,
        Err(e) => {
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
    wait_for_redis(&url, &container_name).await?;
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

/// Remove any leftover `avrag-test-pg-*` / `avrag-test-milvus-*` containers
/// from previous test runs that did not clean up (CI flakes, SIGKILL, OOM).
///
/// Idempotent. Logs a single line per removed container.
pub async fn cleanup_orphaned_test_containers() -> anyhow::Result<usize> {
    let mut removed = 0usize;
    for prefix in ["avrag-test-pg-", "avrag-test-milvus-", "avrag-test-redis-"] {
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
            let status = tokio::process::Command::new("docker")
                .args(["rm", "-f", name.trim()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await?;
            if status.success() {
                eprintln!("[product_e2e] cleaned up orphan container: {name}");
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
    fn parse_docker_port_output_rejects_missing_mapping() {
        let err = parse_docker_port_output("").unwrap_err();

        assert!(err.to_string().contains("missing docker port mapping"));
    }
}
