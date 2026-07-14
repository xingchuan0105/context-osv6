//! Local data-plane health probes + stack ensure for desktop
//! (Postgres / Redis / full Milvus via docker compose).

use serde::Serialize;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::api::IpcApiError;

#[derive(Debug, Clone, Serialize)]
pub struct LocalStackServiceStatus {
    pub id: String,
    pub label: String,
    pub endpoint: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalStackStatus {
    pub overall_ok: bool,
    pub services: Vec<LocalStackServiceStatus>,
    pub compose_hint: String,
    /// Absolute path to `scripts/desktop-local-stack.sh` when monorepo is found.
    pub script_path: Option<String>,
    /// Absolute path to `desktop/runtime/client.env` when monorepo is found.
    pub env_file_path: Option<String>,
    pub env_file_exists: bool,
}

/// Connection strings for local PG / Redis / Milvus (and related process flags).
#[derive(Debug, Clone, Serialize)]
pub struct ClientRuntimeConfig {
    pub database_url: String,
    pub redis_url: String,
    pub milvus_url: String,
    pub pg_host: String,
    pub pg_port: u16,
    pub redis_host: String,
    pub redis_port: u16,
    pub milvus_host: String,
    pub milvus_port: u16,
    pub migrations_dir: Option<String>,
    pub env_file_path: Option<String>,
    pub env_file_exists: bool,
    pub monorepo_root: Option<String>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnsureLocalStackResult {
    pub ok: bool,
    pub message: String,
    pub stdout: String,
    pub stderr: String,
    pub status: LocalStackStatus,
    pub config: ClientRuntimeConfig,
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn probe_tcp(host: &str, port: u16) -> (bool, String) {
    let endpoint = format!("{host}:{port}");
    let addrs = match endpoint.to_socket_addrs() {
        Ok(a) => a.collect::<Vec<SocketAddr>>(),
        Err(e) => return (false, format!("resolve failed: {e}")),
    };
    if addrs.is_empty() {
        return (false, "no addresses".into());
    }
    match TcpStream::connect_timeout(&addrs[0], Duration::from_millis(400)) {
        Ok(_) => (true, "port open".into()),
        Err(e) => (false, e.to_string()),
    }
}

/// Resolve monorepo root for compose script / migrations (dev + monorepo installs).
fn monorepo_root() -> Option<PathBuf> {
    if let Ok(root) = std::env::var("CONTEXT_OS_ROOT") {
        let p = PathBuf::from(root);
        if p.join("scripts/desktop-local-stack.sh").is_file() {
            return Some(p);
        }
    }

    // desktop/src-tauri → ../.. at compile time (works while developing from the tree).
    let mut from_manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // src-tauri → desktop → repo root
    from_manifest.pop();
    from_manifest.pop();
    if from_manifest
        .join("scripts/desktop-local-stack.sh")
        .is_file()
    {
        return Some(from_manifest);
    }

    // Walk cwd upward (useful if launched from monorepo subdir).
    if let Ok(mut cwd) = std::env::current_dir() {
        for _ in 0..8 {
            if cwd.join("scripts/desktop-local-stack.sh").is_file() {
                return Some(cwd);
            }
            if !cwd.pop() {
                break;
            }
        }
    }

    None
}

fn script_path(root: &Path) -> PathBuf {
    root.join("scripts/desktop-local-stack.sh")
}

fn env_file_path(root: &Path) -> PathBuf {
    root.join("desktop/runtime/client.env")
}

fn migrations_dir(root: &Path) -> PathBuf {
    root.join("avrag-rs/migrations")
}

fn default_runtime_endpoints() -> (String, u16, String, u16, String, u16) {
    let pg_host = env_or("CLIENT_PG_HOST", "127.0.0.1");
    let pg_port: u16 = env_or("CLIENT_PG_PORT", "5433").parse().unwrap_or(5433);
    let redis_host = env_or("CLIENT_REDIS_HOST", "127.0.0.1");
    let redis_port: u16 = env_or("CLIENT_REDIS_PORT", "6380").parse().unwrap_or(6380);
    let milvus_host = env_or("CLIENT_MILVUS_HOST", "127.0.0.1");
    let milvus_port: u16 = env_or("CLIENT_MILVUS_PORT", "19530")
        .parse()
        .unwrap_or(19530);
    (pg_host, pg_port, redis_host, redis_port, milvus_host, milvus_port)
}

fn build_status() -> LocalStackStatus {
    let (pg_host, pg_port, redis_host, redis_port, milvus_host, milvus_port) =
        default_runtime_endpoints();

    let mut services = Vec::new();

    let (pg_ok, pg_detail) = probe_tcp(&pg_host, pg_port);
    services.push(LocalStackServiceStatus {
        id: "postgres".into(),
        label: "PostgreSQL".into(),
        endpoint: format!("{pg_host}:{pg_port}"),
        ok: pg_ok,
        detail: pg_detail,
    });

    let (redis_ok, redis_detail) = probe_tcp(&redis_host, redis_port);
    services.push(LocalStackServiceStatus {
        id: "redis".into(),
        label: "Redis".into(),
        endpoint: format!("{redis_host}:{redis_port}"),
        ok: redis_ok,
        detail: redis_detail,
    });

    let (milvus_ok, milvus_detail) = probe_tcp(&milvus_host, milvus_port);
    services.push(LocalStackServiceStatus {
        id: "milvus".into(),
        label: "Milvus".into(),
        endpoint: format!("{milvus_host}:{milvus_port}"),
        ok: milvus_ok,
        detail: milvus_detail,
    });

    let root = monorepo_root();
    let script = root.as_ref().map(|r| script_path(r).display().to_string());
    let env_path = root.as_ref().map(|r| env_file_path(r));
    let env_exists = env_path
        .as_ref()
        .map(|p| p.is_file())
        .unwrap_or(false);

    let overall_ok = services.iter().all(|s| s.ok);
    LocalStackStatus {
        overall_ok,
        services,
        compose_hint: "bash scripts/desktop-local-stack.sh ensure".into(),
        script_path: script,
        env_file_path: env_path.map(|p| p.display().to_string()),
        env_file_exists: env_exists,
    }
}

fn build_runtime_config() -> ClientRuntimeConfig {
    let (pg_host, pg_port, redis_host, redis_port, milvus_host, milvus_port) =
        default_runtime_endpoints();

    let database_url = env_or(
        "DATABASE_URL",
        &format!("postgres://avrag:avrag@{pg_host}:{pg_port}/avrag_client"),
    );
    let redis_url = env_or("REDIS_URL", &format!("redis://{redis_host}:{redis_port}/0"));
    let milvus_url = env_or(
        "MILVUS_URL",
        &format!("http://{milvus_host}:{milvus_port}"),
    );

    let root = monorepo_root();
    let env_path = root.as_ref().map(|r| env_file_path(r));
    let env_exists = env_path
        .as_ref()
        .map(|p| p.is_file())
        .unwrap_or(false);
    let migrations = root.as_ref().map(|r| migrations_dir(r).display().to_string());

    // Prefer values already written to client.env when process env is still default
    // (UI can show the on-disk target even before the shell exports it).
    let note = if root.is_none() {
        "Monorepo root not found. Set CONTEXT_OS_ROOT or run scripts from the repo. Packaged clients will use bundled stack paths later.".into()
    } else if !env_exists {
        "Run ensure_local_stack (or `bash scripts/desktop-local-stack.sh ensure`) to start services, write client.env, and apply migrations.".into()
    } else {
        "Point avrag-api / worker at these URLs (source client.env). Desktop chat remains BYOK; ingest/API attach is the next product step.".into()
    };

    ClientRuntimeConfig {
        database_url,
        redis_url,
        milvus_url,
        pg_host,
        pg_port,
        redis_host,
        redis_port,
        milvus_host,
        milvus_port,
        migrations_dir: migrations,
        env_file_path: env_path.map(|p| p.display().to_string()),
        env_file_exists: env_exists,
        monorepo_root: root.map(|r| r.display().to_string()),
        note,
    }
}

fn run_stack_script(arg: &str) -> Result<(i32, String, String), IpcApiError> {
    let root = monorepo_root().ok_or_else(|| {
        IpcApiError::bad_request(
            "monorepo_not_found",
            "Cannot find scripts/desktop-local-stack.sh. Set CONTEXT_OS_ROOT to the monorepo root, or run: bash scripts/desktop-local-stack.sh ensure",
        )
    })?;
    let script = script_path(&root);
    if !script.is_file() {
        return Err(IpcApiError::bad_request(
            "script_missing",
            format!("Stack script missing: {}", script.display()),
        ));
    }

    let output = Command::new("bash")
        .arg(&script)
        .arg(arg)
        .current_dir(&root)
        .env("CONTEXT_OS_ROOT", root.as_os_str())
        .output()
        .map_err(|e| {
            IpcApiError::internal(format!(
                "Failed to run desktop-local-stack.sh {arg}: {e}. Is Docker installed?"
            ))
        })?;

    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    Ok((code, stdout, stderr))
}

#[tauri::command]
pub fn get_local_stack_status() -> LocalStackStatus {
    build_status()
}

#[tauri::command]
pub fn get_client_runtime_config() -> ClientRuntimeConfig {
    build_runtime_config()
}

/// Bring up compose stack, write `client.env`, apply SQL migrations (`ensure`).
#[tauri::command]
pub async fn ensure_local_stack() -> Result<EnsureLocalStackResult, IpcApiError> {
    let (code, stdout, stderr) =
        tokio::task::spawn_blocking(|| run_stack_script("ensure"))
            .await
            .map_err(|e| IpcApiError::internal(format!("ensure task join error: {e}")))??;

    let status = build_status();
    let config = build_runtime_config();
    let ok = code == 0 && status.overall_ok;
    let message = if code == 0 {
        if status.overall_ok {
            "Local stack is up; client.env written; migrations applied (if sqlx available)."
                .into()
        } else {
            "Script finished but not all ports are open yet — retry probe shortly.".into()
        }
    } else {
        format!(
            "desktop-local-stack.sh ensure failed (exit {code}). {}",
            stderr.lines().last().unwrap_or("see stderr")
        )
    };

    Ok(EnsureLocalStackResult {
        ok,
        message,
        stdout,
        stderr,
        status,
        config,
    })
}

/// Stop compose stack (data volumes retained under desktop/runtime/data).
#[tauri::command]
pub async fn stop_local_stack() -> Result<EnsureLocalStackResult, IpcApiError> {
    let (code, stdout, stderr) = tokio::task::spawn_blocking(|| run_stack_script("down"))
        .await
        .map_err(|e| IpcApiError::internal(format!("stop task join error: {e}")))??;

    let status = build_status();
    let config = build_runtime_config();
    let ok = code == 0;
    let message = if ok {
        "Local stack stopped (data retained).".into()
    } else {
        format!(
            "desktop-local-stack.sh down failed (exit {code}). {}",
            stderr.lines().last().unwrap_or("see stderr")
        )
    };

    Ok(EnsureLocalStackResult {
        ok,
        message,
        stdout,
        stderr,
        status,
        config,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_config_has_offset_ports() {
        let cfg = build_runtime_config();
        assert_eq!(cfg.pg_port, 5433);
        assert_eq!(cfg.redis_port, 6380);
        assert_eq!(cfg.milvus_port, 19530);
        assert!(cfg.database_url.contains("5433"));
        assert!(cfg.redis_url.contains("6380"));
        assert!(cfg.milvus_url.contains("19530"));
    }

    #[test]
    fn status_lists_three_services() {
        let st = build_status();
        assert_eq!(st.services.len(), 3);
        assert!(st.compose_hint.contains("desktop-local-stack"));
    }
}
