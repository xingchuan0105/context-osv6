//! Local product process control (avrag-api + avrag-worker on client data plane).

use serde::Serialize;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::api::IpcApiError;

#[derive(Debug, Clone, Serialize)]
pub struct LocalProductStatus {
    pub overall_ok: bool,
    pub api_ok: bool,
    pub worker_ok: bool,
    pub api_base_url: String,
    pub api_endpoint: String,
    pub health_detail: String,
    pub worker_detail: String,
    pub compose_hint: String,
    pub script_path: Option<String>,
    pub log_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnsureLocalProductResult {
    pub ok: bool,
    pub message: String,
    pub stdout: String,
    pub stderr: String,
    pub status: LocalProductStatus,
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn monorepo_root() -> Option<PathBuf> {
    if let Ok(root) = std::env::var("CONTEXT_OS_ROOT") {
        let p = PathBuf::from(root);
        if p.join("scripts/desktop-local-product.sh").is_file() {
            return Some(p);
        }
    }
    let mut from_manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    from_manifest.pop();
    from_manifest.pop();
    if from_manifest
        .join("scripts/desktop-local-product.sh")
        .is_file()
    {
        return Some(from_manifest);
    }
    if let Ok(mut cwd) = std::env::current_dir() {
        for _ in 0..8 {
            if cwd.join("scripts/desktop-local-product.sh").is_file() {
                return Some(cwd);
            }
            if !cwd.pop() {
                break;
            }
        }
    }
    None
}

fn product_script(root: &Path) -> PathBuf {
    root.join("scripts/desktop-local-product.sh")
}

fn read_env_file_value(key: &str) -> Option<String> {
    let path = monorepo_root()
        .map(|r| r.join("desktop/runtime/client.env"))
        .or_else(|| {
            std::env::var("CONTEXT_OS_CLIENT_HOME")
                .ok()
                .map(|h| PathBuf::from(h).join("client.env"))
        })?;
    let raw = std::fs::read_to_string(path).ok()?;
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key {
                return Some(v.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

fn client_api_base() -> String {
    if let Ok(v) = std::env::var("AVRAG_PUBLIC_BASE_URL") {
        if !v.trim().is_empty() {
            return v.trim().trim_end_matches('/').to_string();
        }
    }
    if let Ok(v) = std::env::var("CLIENT_API_BASE_URL") {
        if !v.trim().is_empty() {
            return v.trim().trim_end_matches('/').to_string();
        }
    }
    if let Some(v) = read_env_file_value("AVRAG_PUBLIC_BASE_URL") {
        return v.trim_end_matches('/').to_string();
    }
    "http://127.0.0.1:18080".into()
}

fn client_api_host_port() -> (String, u16) {
    let host = env_or("CLIENT_API_HOST", "127.0.0.1");
    let port: u16 = env_or("CLIENT_API_PORT", "18080").parse().unwrap_or(18080);
    (host, port)
}

fn probe_tcp(host: &str, port: u16) -> bool {
    let endpoint = format!("{host}:{port}");
    let Ok(addrs) = endpoint.to_socket_addrs() else {
        return false;
    };
    let addrs: Vec<SocketAddr> = addrs.collect();
    if addrs.is_empty() {
        return false;
    }
    TcpStream::connect_timeout(&addrs[0], Duration::from_millis(400)).is_ok()
}

fn pid_alive(pidfile: &Path) -> bool {
    let Ok(raw) = std::fs::read_to_string(pidfile) else {
        return false;
    };
    let pid = raw.trim();
    if pid.is_empty() {
        return false;
    }
    // Prefer /proc on Linux (no extra deps).
    if Path::new(&format!("/proc/{pid}")).exists() {
        return true;
    }
    // Fallback: kill -0 via shell on other Unix-like systems.
    Command::new("kill")
        .args(["-0", pid])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn build_status() -> LocalProductStatus {
    let (host, port) = client_api_host_port();
    let base = client_api_base();
    let root = monorepo_root();
    let script = root.as_ref().map(|r| product_script(r).display().to_string());
    let log_dir = root
        .as_ref()
        .map(|r| r.join("desktop/runtime/logs").display().to_string());
    let worker_pid = root
        .as_ref()
        .map(|r| r.join("desktop/runtime/run/worker.pid"));
    let api_pid = root.as_ref().map(|r| r.join("desktop/runtime/run/api.pid"));

    let port_ok = probe_tcp(&host, port);
    let health_url = format!("{base}/health");
    let (api_ok, health_detail) = if port_ok {
        match ureq_get_health(&health_url) {
            Ok(body) => (true, body),
            Err(e) => (false, format!("port open but /health failed: {e}")),
        }
    } else {
        (false, "API port closed".into())
    };

    let worker_ok = worker_pid
        .as_ref()
        .map(|p| pid_alive(p))
        .unwrap_or(false);
    let worker_detail = if worker_ok {
        let pid = worker_pid
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default();
        format!("running pid {}", pid.trim())
    } else if api_pid.as_ref().map(|p| pid_alive(p)).unwrap_or(false) && !worker_ok {
        "worker pid not alive".into()
    } else {
        "not running".into()
    };

    LocalProductStatus {
        overall_ok: api_ok && worker_ok,
        api_ok,
        worker_ok,
        api_base_url: base,
        api_endpoint: format!("{host}:{port}"),
        health_detail,
        worker_detail,
        compose_hint: "bash scripts/desktop-local-product.sh ensure".into(),
        script_path: script,
        log_dir,
    }
}

/// Health probe via curl (always available on our WSL/Linux target; Windows later).
fn ureq_get_health(url: &str) -> Result<String, String> {
    let output = Command::new("curl")
        .args(["-fsS", "--max-time", "2", url])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(if err.trim().is_empty() {
            format!("curl exit {}", output.status)
        } else {
            err.trim().to_string()
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn run_product_script(arg: &str) -> Result<(i32, String, String), IpcApiError> {
    let root = monorepo_root().ok_or_else(|| {
        IpcApiError::bad_request(
            "monorepo_not_found",
            "Cannot find scripts/desktop-local-product.sh. Set CONTEXT_OS_ROOT.",
        )
    })?;
    let script = product_script(&root);
    if !script.is_file() {
        return Err(IpcApiError::bad_request(
            "script_missing",
            format!("Product script missing: {}", script.display()),
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
                "Failed to run desktop-local-product.sh {arg}: {e}"
            ))
        })?;
    Ok((
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    ))
}

#[tauri::command]
pub fn get_local_product_status() -> LocalProductStatus {
    build_status()
}

#[tauri::command]
pub async fn ensure_local_product() -> Result<EnsureLocalProductResult, IpcApiError> {
    let (code, stdout, stderr) =
        tokio::task::spawn_blocking(|| run_product_script("ensure"))
            .await
            .map_err(|e| IpcApiError::internal(format!("ensure product join: {e}")))??;

    let status = build_status();
    let ok = code == 0 && status.api_ok;
    let message = if code == 0 {
        if status.api_ok {
            format!(
                "Local product API ready at {} (worker: {}).",
                status.api_base_url,
                if status.worker_ok { "up" } else { "check logs" }
            )
        } else {
            "Script finished but API health not ready — check desktop/runtime/logs/api.log".into()
        }
    } else {
        format!(
            "desktop-local-product.sh ensure failed (exit {code}). {}",
            stderr.lines().last().unwrap_or("see stderr")
        )
    };

    Ok(EnsureLocalProductResult {
        ok,
        message,
        stdout,
        stderr,
        status,
    })
}

#[tauri::command]
pub async fn stop_local_product() -> Result<EnsureLocalProductResult, IpcApiError> {
    let (code, stdout, stderr) = tokio::task::spawn_blocking(|| run_product_script("stop"))
        .await
        .map_err(|e| IpcApiError::internal(format!("stop product join: {e}")))??;

    let status = build_status();
    let ok = code == 0 && !status.api_ok;
    let message = if code == 0 {
        "Local product API/worker stopped.".into()
    } else {
        format!(
            "desktop-local-product.sh stop failed (exit {code}). {}",
            stderr.lines().last().unwrap_or("see stderr")
        )
    };

    Ok(EnsureLocalProductResult {
        ok,
        message,
        stdout,
        stderr,
        status,
    })
}

/// Base URL used by desktop `api_call` HTTP proxy.
pub fn product_api_base_url() -> String {
    client_api_base()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_api_port_is_offset() {
        let (host, port) = client_api_host_port();
        assert_eq!(host, "127.0.0.1");
        assert_eq!(port, 18080);
        assert!(client_api_base().contains("18080"));
    }
}
