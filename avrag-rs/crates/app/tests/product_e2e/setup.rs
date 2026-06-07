//! Test infrastructure setup — Docker-based Postgres + Milvus + ephemeral HTTP server + worker spawn.
//!
//! Design: minimal external dependencies (no testcontainers crate), uses docker CLI directly.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Postgres
// ---------------------------------------------------------------------------

/// Start a Postgres container via docker and return its connection URL.
///
/// Container is named `avrag-test-pg-{port}` and auto-removed on stop.
pub async fn start_postgres() -> anyhow::Result<String> {
    let port = find_ephemeral_port()?;
    let container_name = format!("avrag-test-pg-{port}");

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
            &format!("{port}:5432"),
            "postgres:16-alpine",
        ])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("docker run postgres failed: {stderr}");
    }

    let url = format!("postgres://test:test@127.0.0.1:{port}/test");
    wait_for_postgres(&url, &container_name).await?;
    Ok(url)
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

fn find_ephemeral_port() -> anyhow::Result<u16> {
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

/// Create a temporary object store directory.
pub fn create_temp_object_store() -> tempfile::TempDir {
    tempfile::tempdir().expect("create tempdir")
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

/// Reuse an existing Milvus instance or start a standalone container.
///
/// First probes `127.0.0.1:19530` via TCP connect. If a Milvus is already
/// running (e.g. from docker-compose), returns its URL directly with
/// `is_external = true`. Otherwise starts a temporary standalone container
/// with `is_external = false`.
pub async fn start_milvus() -> anyhow::Result<MilvusInstance> {
    let url = "http://127.0.0.1:19530";
    // Fast-path: check if port 19530 is open
    if std::net::TcpStream::connect_timeout(
        &"127.0.0.1:19530".parse().unwrap(),
        std::time::Duration::from_secs(2),
    )
    .is_ok()
    {
        return Ok(MilvusInstance {
            url: url.to_string(),
            container_name: None,
            is_external: true,
        });
    }

    let grpc_port = find_ephemeral_port()?;
    let container_name = format!("avrag-test-milvus-{grpc_port}");

    let output = tokio::process::Command::new("docker")
        .args([
            "run",
            "-d",
            "--rm",
            "--name",
            &container_name,
            "-p",
            &format!("{grpc_port}:19530"),
            "milvusdb/milvus:v2.4.5",
            "milvus",
            "run",
            "standalone",
        ])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("docker run milvus failed: {stderr}");
    }

    let url = format!("http://127.0.0.1:{grpc_port}");
    wait_for_milvus(&url, &container_name).await?;
    Ok(MilvusInstance {
        url,
        container_name: Some(container_name),
        is_external: false,
    })
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

async fn wait_for_milvus(url: &str, container_name: &str) -> anyhow::Result<()> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(180);
    loop {
        let port = url.rsplit(':').next().unwrap_or("19530");
        if std::net::TcpStream::connect_timeout(
            &format!("127.0.0.1:{port}").parse().unwrap(),
            std::time::Duration::from_secs(2),
        )
        .is_ok()
        {
            return Ok(());
        }
        if tokio::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(1000)).await;
        } else {
            let _ = stop_milvus(container_name).await;
            anyhow::bail!("milvus did not become ready in 180s");
        }
    }
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
    for prefix in ["avrag-test-pg-", "avrag-test-milvus-"] {
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
