//! Local data-plane health probes + stack ensure for desktop
//! (Postgres+pgvector + Redis via docker compose; no Milvus).

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
    /// Nested Docker probe for install guidance in settings UI.
    pub docker: Option<super::docker_status::DockerStatus>,
}

/// Connection strings for local PG / Redis and retrieval backend flags.
#[derive(Debug, Clone, Serialize)]
pub struct ClientRuntimeConfig {
    pub database_url: String,
    pub redis_url: String,
    /// Desktop default is `pgvector` (storage-pgvector). Cloud SaaS uses milvus.
    pub retrieval_backend: String,
    /// Legacy field: unused on slim desktop stack (kept for older UI JSON).
    #[serde(default)]
    pub milvus_url: String,
    pub pg_host: String,
    pub pg_port: u16,
    pub redis_host: String,
    pub redis_port: u16,
    /// Legacy: always empty / unused on slim stack.
    #[serde(default)]
    pub milvus_host: String,
    /// Legacy: always 0 on slim stack.
    #[serde(default)]
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

/// Strip URL passwords before returning connection strings over IPC.
fn redact_url_credentials(raw: &str) -> String {
    let Ok(mut parsed) = url::Url::parse(raw) else {
        return raw.to_string();
    };
    if parsed.password().is_some() {
        let _ = parsed.set_password(Some("***"));
    }
    parsed.to_string()
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
fn script_path(root: &Path) -> PathBuf {
    root.join("scripts").join("desktop-local-stack.sh")
}

fn is_live_monorepo_root(root: &Path) -> bool {
    root.is_dir() && script_path(root).is_file()
}

fn monorepo_root() -> Option<PathBuf> {
    if let Ok(root) = std::env::var("CONTEXT_OS_ROOT") {
        let p = PathBuf::from(root);
        if is_live_monorepo_root(&p) {
            return Some(p);
        }
    }

    // Compile-time path only when that checkout exists on this host (not a
    // cross-compiled Windows PE carrying a WSL `CARGO_MANIFEST_DIR`).
    let mut from_manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // src-tauri → desktop → repo root
    from_manifest.pop();
    from_manifest.pop();
    if is_live_monorepo_root(&from_manifest) {
        return Some(from_manifest);
    }

    // Walk cwd upward (useful if launched from monorepo subdir).
    if let Ok(mut cwd) = std::env::current_dir() {
        for _ in 0..8 {
            if is_live_monorepo_root(&cwd) {
                return Some(cwd);
            }
            if !cwd.pop() {
                break;
            }
        }
    }

    None
}

fn env_file_path(root: &Path) -> PathBuf {
    root.join("desktop/runtime/client.env")
}

fn migrations_dir(root: &Path) -> PathBuf {
    root.join("avrag-rs/migrations")
}

fn default_runtime_endpoints() -> (String, u16, String, u16) {
    let pg_host = env_or("CLIENT_PG_HOST", "127.0.0.1");
    let pg_port: u16 = env_or("CLIENT_PG_PORT", "5433").parse().unwrap_or(5433);
    let redis_host = env_or("CLIENT_REDIS_HOST", "127.0.0.1");
    let redis_port: u16 = env_or("CLIENT_REDIS_PORT", "6380").parse().unwrap_or(6380);
    (pg_host, pg_port, redis_host, redis_port)
}

fn build_status() -> LocalStackStatus {
    let (pg_host, pg_port, redis_host, redis_port) = default_runtime_endpoints();

    let mut services = Vec::new();

    let (pg_ok, pg_detail) = probe_tcp(&pg_host, pg_port);
    services.push(LocalStackServiceStatus {
        id: "postgres".into(),
        label: "PostgreSQL + pgvector".into(),
        endpoint: format!("{pg_host}:{pg_port}"),
        ok: pg_ok,
        detail: if pg_ok {
            "port open (control plane + VGRAG retrieval)".into()
        } else {
            pg_detail
        },
    });

    let (redis_ok, redis_detail) = probe_tcp(&redis_host, redis_port);
    services.push(LocalStackServiceStatus {
        id: "redis".into(),
        label: "Redis".into(),
        endpoint: format!("{redis_host}:{redis_port}"),
        ok: redis_ok,
        detail: redis_detail,
    });

    let root = monorepo_root();
    let script = root.as_ref().map(|r| script_path(r).display().to_string());
    let env_path = root.as_ref().map(|r| env_file_path(r));
    let env_exists = env_path
        .as_ref()
        .map(|p| p.is_file())
        .unwrap_or(false);

    // Slim stack: only PG + Redis are required for overall_ok.
    let overall_ok = services.iter().all(|s| s.ok);
    let docker = Some(super::docker_status::docker_status_snapshot());
    LocalStackStatus {
        overall_ok,
        services,
        compose_hint: "ensure_local_stack IPC (Rust native) or bash scripts/desktop-local-stack.sh ensure"
            .into(),
        script_path: script,
        env_file_path: env_path.map(|p| p.display().to_string()),
        env_file_exists: env_exists,
        docker,
    }
}

fn build_runtime_config() -> ClientRuntimeConfig {
    let (pg_host, pg_port, redis_host, redis_port) = default_runtime_endpoints();

    let database_url = env_or(
        "DATABASE_URL",
        &format!("postgres://avrag:avrag@{pg_host}:{pg_port}/avrag_client"),
    );
    let redis_url = env_or("REDIS_URL", &format!("redis://{redis_host}:{redis_port}/0"));
    let retrieval_backend = env_or("RETRIEVAL_BACKEND", "pgvector");

    let root = monorepo_root();
    let env_path = root.as_ref().map(|r| env_file_path(r));
    let env_exists = env_path
        .as_ref()
        .map(|p| p.is_file())
        .unwrap_or(false);
    let migrations = root.as_ref().map(|r| migrations_dir(r).display().to_string());

    let note = if root.is_none() {
        "Monorepo root not found. Set CONTEXT_OS_ROOT or run scripts from the repo. Packaged clients will use bundled stack paths later.".into()
    } else if !env_exists {
        "Run ensure_local_stack (or `bash scripts/desktop-local-stack.sh ensure`) to start native Postgres+pgvector + Redis (no Docker), write client.env, and apply migrations.".into()
    } else {
        "Data plane ready (STACK_MODE prefer native, RETRIEVAL_BACKEND=pgvector). Start product with bash scripts/desktop-local-product.sh ensure (API :18080). Desktop chat remains BYOK; REST via api_call.".into()
    };

    ClientRuntimeConfig {
        database_url: redact_url_credentials(&database_url),
        redis_url: redact_url_credentials(&redis_url),
        retrieval_backend,
        milvus_url: String::new(),
        pg_host,
        pg_port,
        redis_host,
        redis_port,
        milvus_host: String::new(),
        milvus_port: 0,
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

    let mut bash = Command::new("bash");
    bash.arg(&script)
        .arg(arg)
        .current_dir(&root)
        .env("CONTEXT_OS_ROOT", root.as_os_str());
    super::win_cmd::hide_console(&mut bash);
    let output = bash.output()
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

/// Bring up data plane: **Rust native first** (no bash/Docker), then bash script fallback.
/// Hard timeout so a stuck `pg_ctl`/pipe cannot freeze the UI forever.
#[tauri::command]
pub async fn ensure_local_stack() -> Result<EnsureLocalStackResult, IpcApiError> {
    let docker = super::docker_status::docker_status_snapshot();
    const NATIVE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);

    // 1) Pure-Rust native path when pg_ctl + redis-server are available.
    if super::native_stack::native_tools_available() {
        let report = match tokio::time::timeout(
            NATIVE_TIMEOUT,
            tokio::task::spawn_blocking(super::native_stack::ensure_native),
        )
        .await
        {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                return Err(IpcApiError::internal(format!("native ensure join: {e}")));
            }
            Err(_) => {
                let status = build_status();
                let config = build_runtime_config();
                return Ok(EnsureLocalStackResult {
                    ok: false,
                    message: format!(
                        "本机数据面启动超时（{secs}s）。请查看 %LOCALAPPDATA%\\Context-OS Client\\logs\\ensure-native.log 与 postgres-native.log",
                        secs = NATIVE_TIMEOUT.as_secs()
                    ),
                    stdout: String::new(),
                    stderr: "timeout".into(),
                    status,
                    config,
                });
            }
        };
        let status = build_status();
        let config = build_runtime_config();
        if report.ok && status.overall_ok {
            return Ok(EnsureLocalStackResult {
                ok: true,
                message: format!(
                    "本机数据面已就绪（Rust native，无 Docker/bash）。{}",
                    report.message
                ),
                stdout: report.log,
                stderr: String::new(),
                status,
                config,
            });
        }
        // Soft-fail into bash fallback only on non-Windows monorepo hosts.
        // Packaged Windows must never invoke WSL/Git-bash paths (hangs or bad paths).
        #[cfg(windows)]
        {
            return Ok(EnsureLocalStackResult {
                ok: false,
                message: format!(
                    "本机数据面未就绪：{}。日志：ensure-native.log / postgres-native.log",
                    report.message
                ),
                stdout: report.log,
                stderr: String::new(),
                status,
                config,
            });
        }
        #[cfg(not(windows))]
        {
        let native_log = report.log;
        let native_msg = report.message;
        if let Ok((code, stdout, stderr)) =
            tokio::task::spawn_blocking(|| run_stack_script("ensure"))
                .await
                .map_err(|e| IpcApiError::internal(format!("ensure task join error: {e}")))?
        {
            let status = build_status();
            let config = build_runtime_config();
            let ok = code == 0 && status.overall_ok;
            return Ok(EnsureLocalStackResult {
                ok,
                message: if ok {
                    format!("bash ensure 成功（native 先试：{native_msg}）")
                } else {
                    format!(
                        "native 与 bash ensure 均未完全就绪：native={native_msg}; bash exit={code}"
                    )
                },
                stdout: format!("--- native ---\n{native_log}\n--- bash ---\n{stdout}"),
                stderr,
                status,
                config,
            });
        }
        }
    }

    // 2) Bash script (auto native/docker) — Unix monorepo only.
    #[cfg(windows)]
    {
        let _ = docker;
        let status = build_status();
        let config = build_runtime_config();
        return Ok(EnsureLocalStackResult {
            ok: false,
            message: "本机数据面工具不可用（未找到 pg_ctl/redis-server）。请重装客户端以恢复 runtime/pgsql 与 runtime/redis。".into(),
            stdout: String::new(),
            stderr: String::new(),
            status,
            config,
        });
    }

    #[cfg(not(windows))]
    {
    let script_result = tokio::task::spawn_blocking(|| run_stack_script("ensure")).await;
    match script_result {
        Ok(Ok((code, stdout, stderr))) => {
            let status = build_status();
            let config = build_runtime_config();
            let ok = code == 0 && status.overall_ok;
            let message = if code == 0 {
                if status.overall_ok {
                    "本机数据面已就绪（脚本 ensure）；client.env 已写入。".into()
                } else {
                    "脚本结束但端口尚未全部开放 — 请稍后重新探测。".into()
                }
            } else {
                let tail = stderr
                    .lines()
                    .chain(stdout.lines())
                    .rev()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("see stderr");
                let hint = if !docker.overall_ok && !super::native_stack::native_tools_available()
                {
                    " · 请安装 postgresql-16 + pgvector + redis-server，或 STACK_MODE=docker"
                } else {
                    ""
                };
                format!("desktop-local-stack.sh ensure failed (exit {code}). {tail}{hint}")
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
        Ok(Err(e)) => {
            // No bash / no monorepo script
            let status = build_status();
            let config = build_runtime_config();
            Ok(EnsureLocalStackResult {
                ok: false,
                message: format!(
                    "无法 ensure：{}。请安装本机 PostgreSQL+pgvector 与 Redis，或在 monorepo 中运行 scripts/desktop-local-stack.sh。",
                    e.message
                ),
                stdout: String::new(),
                stderr: e.message,
                status,
                config,
            })
        }
        Err(e) => Err(IpcApiError::internal(format!("ensure task join error: {e}"))),
    }
    }
}

/// Stop data plane: native stop + optional bash down.
#[tauri::command]
pub async fn stop_local_stack() -> Result<EnsureLocalStackResult, IpcApiError> {
    let native = tokio::task::spawn_blocking(super::native_stack::stop_native)
        .await
        .map_err(|e| IpcApiError::internal(format!("native stop join: {e}")))?;

    let mut stdout = format!("--- native ---\n{}\n", native.log);
    let mut stderr = String::new();
    let mut bash_ok = true;
    if monorepo_root().is_some() {
        match tokio::task::spawn_blocking(|| run_stack_script("down")).await {
            Ok(Ok((code, out, err))) => {
                stdout.push_str("--- bash ---\n");
                stdout.push_str(&out);
                stderr = err;
                bash_ok = code == 0;
            }
            Ok(Err(_)) => {}
            Err(_) => {}
        }
    }

    let status = build_status();
    let config = build_runtime_config();
    let ok = bash_ok || native.ok;
    Ok(EnsureLocalStackResult {
        ok,
        message: if !status.overall_ok {
            "本机数据面已停止（或端口已释放）。".into()
        } else {
            "stop 已调用，但仍有端口在监听（可能是系统 Redis）。".into()
        },
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
        assert_eq!(cfg.retrieval_backend, "pgvector");
        assert!(cfg.database_url.contains("5433"));
        assert!(cfg.redis_url.contains("6380"));
        assert!(cfg.milvus_url.is_empty());
        assert_eq!(cfg.milvus_port, 0);
    }

    #[test]
    fn runtime_config_redacts_url_passwords() {
        let redacted = redact_url_credentials("postgres://avrag:s3cret@127.0.0.1:5433/avrag_client");
        assert!(redacted.contains("5433"));
        assert!(redacted.contains(":***@"));
        assert!(!redacted.contains("s3cret"));
        let redis = redact_url_credentials("redis://:hunter2@127.0.0.1:6380/0");
        assert!(redis.contains("6380"));
        assert!(!redis.contains("hunter2"));
    }

    #[test]
    fn status_lists_pg_and_redis_only() {
        let st = build_status();
        assert_eq!(st.services.len(), 2);
        assert_eq!(st.services[0].id, "postgres");
        assert_eq!(st.services[1].id, "redis");
        assert!(st.compose_hint.contains("desktop-local-stack"));
        assert!(!st.services.iter().any(|s| s.id == "milvus"));
    }
}
