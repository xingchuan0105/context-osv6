//! Local product process control (avrag-api + avrag-worker on client data plane).
//!
//! Cold start prefers a pure-Rust path (spawn sidecars + client.env) so installed
//! clients do not need monorepo bash. Bash script remains a monorepo fallback.

use serde::Serialize;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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
    /// Resolved avrag-api path (NSIS externalBin, CLIENT_HOME, or monorepo runtime/bin).
    pub api_bin_path: Option<String>,
    pub worker_bin_path: Option<String>,
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

fn product_script(root: &Path) -> PathBuf {
    // Prefer multi-component join (avoids Windows path glitches with "a/b" strings).
    root.join("scripts").join("desktop-local-product.sh")
}

/// True when `root` is a real monorepo checkout on **this** host (not a WSL path
/// baked into a Windows PE via `CARGO_MANIFEST_DIR` at cross-compile time).
fn is_live_monorepo_root(root: &Path) -> bool {
    root.is_dir() && product_script(root).is_file()
}

fn monorepo_root() -> Option<PathBuf> {
    if let Ok(root) = std::env::var("CONTEXT_OS_ROOT") {
        let p = PathBuf::from(root);
        if is_live_monorepo_root(&p) {
            return Some(p);
        }
    }
    // Compile-time path only works when developing on the same OS that built the binary.
    let mut from_manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    from_manifest.pop(); // desktop
    from_manifest.pop(); // repo root
    if is_live_monorepo_root(&from_manifest) {
        return Some(from_manifest);
    }
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

/// State dir for client.env / run / logs (install AppData or monorepo desktop/runtime).
fn state_runtime_dir() -> Option<PathBuf> {
    super::native_stack::runtime_home()
}

/// Sidecars bundled via Tauri externalBin land next to the main executable.
fn sidecar_next_to_exe(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let candidates = [
        dir.join(name),
        dir.join(format!("{name}.exe")),
        dir.join("bin").join(name),
        dir.join("bin").join(format!("{name}.exe")),
        dir.join("resources").join("bin").join(name),
        dir.join("resources").join("bin").join(format!("{name}.exe")),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

fn bin_candidate(dir: &Path, name: &str) -> Option<PathBuf> {
    let unix = dir.join(name);
    if unix.is_file() {
        return Some(unix);
    }
    let win = dir.join(format!("{name}.exe"));
    if win.is_file() {
        return Some(win);
    }
    None
}

/// Prefer installed sidecar, then runtime/bin, monorepo staged bin, cargo targets.
pub fn resolve_product_bin(name: &str) -> Option<PathBuf> {
    if let Some(p) = sidecar_next_to_exe(name) {
        return Some(p);
    }
    if let Ok(home) = std::env::var("CONTEXT_OS_CLIENT_HOME") {
        if let Some(p) = bin_candidate(&PathBuf::from(home).join("bin"), name) {
            return Some(p);
        }
    }
    if let Some(rt) = state_runtime_dir() {
        if let Some(p) = bin_candidate(&rt.join("bin"), name) {
            return Some(p);
        }
    }
    if let Some(root) = monorepo_root() {
        if let Some(p) = bin_candidate(&root.join("desktop/runtime/bin"), name) {
            return Some(p);
        }
        for rel in [
            "avrag-rs/target/release",
            "avrag-rs/target/debug",
        ] {
            if let Some(p) = bin_candidate(&root.join(rel), name) {
                return Some(p);
            }
        }
    }
    None
}

fn client_env_path() -> Option<PathBuf> {
    if let Some(rt) = state_runtime_dir() {
        let p = rt.join("client.env");
        if p.is_file() {
            return Some(p);
        }
    }
    if let Ok(home) = std::env::var("CONTEXT_OS_CLIENT_HOME") {
        let p = PathBuf::from(home).join("client.env");
        if p.is_file() {
            return Some(p);
        }
    }
    monorepo_root().map(|r| r.join("desktop/runtime/client.env"))
}

fn read_env_file_value(key: &str) -> Option<String> {
    let path = client_env_path()?;
    let raw = fs::read_to_string(path).ok()?;
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

fn parse_env_file(path: &Path) -> Vec<(String, String)> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let key = k.trim();
            if key.is_empty() {
                continue;
            }
            out.push((
                key.to_string(),
                v.trim().trim_matches('"').to_string(),
            ));
        }
    }
    out
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
    let host = read_env_file_value("CLIENT_API_HOST")
        .or_else(|| std::env::var("CLIENT_API_HOST").ok())
        .unwrap_or_else(|| "127.0.0.1".into());
    let port: u16 = read_env_file_value("CLIENT_API_PORT")
        .or_else(|| std::env::var("CLIENT_API_PORT").ok())
        .unwrap_or_else(|| env_or("CLIENT_API_PORT", "18080"))
        .parse()
        .unwrap_or(18080);
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
    let Ok(raw) = fs::read_to_string(pidfile) else {
        return false;
    };
    let pid = raw.trim();
    if pid.is_empty() {
        return false;
    }
    if Path::new(&format!("/proc/{pid}")).exists() {
        return true;
    }
    #[cfg(unix)]
    {
        return Command::new("kill")
            .args(["-0", pid])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    }
    #[cfg(windows)]
    {
        // Best-effort: tasklist is heavy; treat non-empty pid file as maybe alive.
        // Health probe is authoritative for API.
        let _ = pid;
        return true;
    }
    #[cfg(not(any(unix, windows)))]
    {
        false
    }
}

fn stop_pidfile(pidfile: &Path) {
    if let Ok(raw) = fs::read_to_string(pidfile) {
        let pid = raw.trim();
        if !pid.is_empty() {
            #[cfg(unix)]
            {
                let _ = Command::new("kill")
                    .arg(pid)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
                thread::sleep(Duration::from_millis(200));
                let _ = Command::new("kill")
                    .args(["-9", pid])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
            #[cfg(windows)]
            {
                if let Ok(p) = pid.parse::<u32>() {
                    let _ = super::win_cmd::kill_pid_tree(p);
                }
            }
        }
    }
    let _ = fs::remove_file(pidfile);
}

fn run_log_dir() -> (PathBuf, PathBuf) {
    if let Some(rt) = state_runtime_dir() {
        return (rt.join("run"), rt.join("logs"));
    }
    if let Some(root) = monorepo_root() {
        let rt = root.join("desktop/runtime");
        return (rt.join("run"), rt.join("logs"));
    }
    (
        PathBuf::from("desktop/runtime/run"),
        PathBuf::from("desktop/runtime/logs"),
    )
}

/// Logs dir (api.log / worker.log). `pub(crate)` for the 数据/诊断 open-logs command.
pub(crate) fn log_dir_path() -> PathBuf {
    run_log_dir().1
}

fn build_status() -> LocalProductStatus {
    let (host, port) = client_api_host_port();
    let base = client_api_base();
    let root = monorepo_root();
    let script = root.as_ref().map(|r| product_script(r).display().to_string());
    let (run_dir, log_dir) = run_log_dir();
    let worker_pid = run_dir.join("worker.pid");
    let api_pid = run_dir.join("api.pid");

    let port_ok = probe_tcp(&host, port);
    let health_url = format!("{base}/health");
    let (api_ok, health_detail) = if port_ok {
        match probe_health(&health_url) {
            Ok(body) => (true, body),
            Err(e) => (false, format!("port open but /health failed: {e}")),
        }
    } else {
        (false, "API port closed".into())
    };

    let worker_ok = pid_alive(&worker_pid);
    let worker_detail = if worker_ok {
        let pid = fs::read_to_string(&worker_pid).unwrap_or_default();
        format!("running pid {}", pid.trim())
    } else if pid_alive(&api_pid) && !worker_ok {
        "worker pid not alive".into()
    } else {
        "not running".into()
    };

    let api_bin = resolve_product_bin("avrag-api").map(|p| p.display().to_string());
    let worker_bin = resolve_product_bin("avrag-worker").map(|p| p.display().to_string());

    LocalProductStatus {
        overall_ok: api_ok && worker_ok,
        api_ok,
        worker_ok,
        api_base_url: base,
        api_endpoint: format!("{host}:{port}"),
        health_detail,
        worker_detail,
        compose_hint: "ensure_local_product IPC (Rust native, bash fallback)".into(),
        script_path: script,
        log_dir: Some(log_dir.display().to_string()),
        api_bin_path: api_bin,
        worker_bin_path: worker_bin,
    }
}

fn probe_health(url: &str) -> Result<String, String> {
    let parsed = url::Url::parse(url).map_err(|e| e.to_string())?;
    let host = parsed.host_str().unwrap_or("127.0.0.1");
    let port = parsed.port_or_known_default().unwrap_or(80);
    let path = if parsed.path().is_empty() { "/" } else { parsed.path() };
    let addr = format!("{host}:{port}");
    let sock = addr
        .to_socket_addrs()
        .map_err(|e| e.to_string())?
        .next()
        .ok_or_else(|| "no address".to_string())?;
    let mut stream =
        TcpStream::connect_timeout(&sock, Duration::from_millis(400)).map_err(|e| e.to_string())?;
    stream.set_read_timeout(Some(Duration::from_millis(800))).ok();
    let req = format!("GET {path} HTTP/1.0\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&buf);
    Ok(text
        .split("\r\n\r\n")
        .nth(1)
        .unwrap_or(text.as_ref())
        .trim()
        .to_string())
}

fn api_healthy() -> bool {
    let (host, port) = client_api_host_port();
    probe_tcp(&host, port)
}

fn open_append_log(path: &Path) -> Result<File, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("open log {}: {e}", path.display()))
}

fn spawn_with_env(
    bin: &Path,
    env_pairs: &[(String, String)],
    log_path: &Path,
    pid_path: &Path,
    cwd: Option<&Path>,
) -> Result<(), String> {
    let log = open_append_log(log_path)?;
    let log_err = open_append_log(log_path)?;
    let mut cmd = Command::new(bin);
    // Prefer binary directory as cwd so Windows LoadLibrary finds MinGW DLLs
    // (libstdc++-6.dll etc.) next to avrag-api.exe / Context-OS.exe.
    let bin_dir = bin.parent();
    if let Some(dir) = cwd.or(bin_dir) {
        cmd.current_dir(dir);
    }
    for (k, v) in env_pairs {
        cmd.env(k, v);
    }
    // Prepend install / runtime dirs to PATH for MinGW + portable libs.
    {
        let mut path_prefix: Vec<PathBuf> = Vec::new();
        if let Some(d) = bin_dir {
            path_prefix.push(d.to_path_buf());
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(d) = exe.parent() {
                path_prefix.push(d.to_path_buf());
                path_prefix.push(d.join("runtime").join("mingw"));
            }
        }
        if let Some(rt) = state_runtime_dir() {
            path_prefix.push(rt.join("runtime").join("mingw"));
            path_prefix.push(rt.join("mingw"));
            path_prefix.push(rt.join("bin"));
        }
        let mut parts: Vec<String> = path_prefix
            .into_iter()
            .filter(|p| p.is_dir() || p.is_file())
            .map(|p| p.display().to_string())
            .collect();
        if let Ok(existing) = std::env::var("PATH") {
            parts.push(existing);
        }
        if !parts.is_empty() {
            let sep = if cfg!(windows) { ";" } else { ":" };
            cmd.env("PATH", parts.join(sep));
        }
    }
    // Desktop isolation defaults (client.env should already set these).
    if !env_pairs.iter().any(|(k, _)| k == "RETRIEVAL_BACKEND") {
        cmd.env("RETRIEVAL_BACKEND", "pgvector");
    }
    // Native stack writes AVRAG_RUN_MIGRATIONS=true for first boot; honor that
    // unless an explicit product override is present. This keeps new STATE_HOME
    // databases migrated before local session/API routes are exercised.
    cmd.env(
        "AVRAG_RUN_MIGRATIONS",
        env_pairs
            .iter()
            .find(|(k, _)| k == "AVRAG_RUN_MIGRATIONS_PRODUCT")
            .or_else(|| env_pairs.iter().find(|(k, _)| k == "AVRAG_RUN_MIGRATIONS"))
            .map(|(_, v)| v.as_str())
            .unwrap_or("false"),
    );
    cmd.stdout(Stdio::from(log));
    cmd.stderr(Stdio::from(log_err));
    cmd.stdin(Stdio::null());

    super::win_cmd::hide_and_detach(&mut cmd);

    let child = cmd
        .spawn()
        .map_err(|e| format!("spawn {}: {e}", bin.display()))?;
    if let Some(parent) = pid_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(pid_path, format!("{}\n", child.id())).map_err(|e| e.to_string())?;
    // Detach: drop Child without wait so process keeps running.
    std::mem::forget(child);
    Ok(())
}

fn wait_api_healthy(secs: u64, log: &mut String) -> bool {
    let deadline = Instant::now() + Duration::from_secs(secs);
    let (host, port) = client_api_host_port();
    let base = client_api_base();
    let url = format!("{base}/health");
    while Instant::now() < deadline {
        // Prefer TCP first — Windows curl can stall and make cold-start feel frozen.
        if probe_tcp(&host, port) {
            if probe_health(&url).is_ok() {
                log.push_str("API healthy\n");
                return true;
            }
            // Port open is enough for desktop bootstrap (migrations may still settle).
            log.push_str("API port open (treating as ready)\n");
            return true;
        }
        thread::sleep(Duration::from_millis(400));
    }
    log.push_str("timeout waiting for API health\n");
    false
}

/// Pure-Rust product bring-up for install + monorepo (no bash required).
fn ensure_product_native() -> Result<String, String> {
    let mut log = String::new();
    if api_healthy() {
        return Ok("product API already healthy".into());
    }

    let state = state_runtime_dir().ok_or_else(|| {
        "runtime state dir not found (install layout or CONTEXT_OS_CLIENT_HOME / monorepo desktop/runtime)"
            .to_string()
    })?;
    let env_path = state.join("client.env");
    if !env_path.is_file() {
        return Err(format!(
            "missing {} — start data stack first (ensure_local_stack)",
            env_path.display()
        ));
    }

    let pg_host = read_env_file_value("CLIENT_PG_HOST").unwrap_or_else(|| "127.0.0.1".into());
    let pg_port: u16 = read_env_file_value("CLIENT_PG_PORT")
        .unwrap_or_else(|| "5433".into())
        .parse()
        .unwrap_or(5433);
    let redis_host = read_env_file_value("CLIENT_REDIS_HOST").unwrap_or_else(|| "127.0.0.1".into());
    let redis_port: u16 = read_env_file_value("CLIENT_REDIS_PORT")
        .unwrap_or_else(|| "6380".into())
        .parse()
        .unwrap_or(6380);
    if !probe_tcp(&pg_host, pg_port) {
        return Err(format!(
            "Postgres not up on {pg_host}:{pg_port} — ensure_local_stack first"
        ));
    }
    if !probe_tcp(&redis_host, redis_port) {
        return Err(format!(
            "Redis not up on {redis_host}:{redis_port} — ensure_local_stack first"
        ));
    }

    let api_bin = resolve_product_bin("avrag-api").ok_or_else(|| {
        "avrag-api binary not found (expected next to app, or desktop/runtime/bin, or cargo target)"
            .to_string()
    })?;
    let worker_bin = resolve_product_bin("avrag-worker").ok_or_else(|| {
        "avrag-worker binary not found (expected next to app, or desktop/runtime/bin, or cargo target)"
            .to_string()
    })?;
    log.push_str(&format!("api_bin={}\n", api_bin.display()));
    log.push_str(&format!("worker_bin={}\n", worker_bin.display()));

    let env_pairs = parse_env_file(&env_path);
    if env_pairs.is_empty() {
        return Err(format!("empty or unreadable env file {}", env_path.display()));
    }

    let (run_dir, log_dir) = run_log_dir();
    fs::create_dir_all(&run_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&log_dir).map_err(|e| e.to_string())?;

    let api_pid = run_dir.join("api.pid");
    let worker_pid = run_dir.join("worker.pid");
    let api_log = log_dir.join("api.log");
    let worker_log = log_dir.join("worker.log");

    // Restart if not healthy.
    if !api_healthy() {
        stop_pidfile(&api_pid);
        log.push_str("starting avrag-api\n");
        spawn_with_env(&api_bin, &env_pairs, &api_log, &api_pid, None)?;
    }
    if !pid_alive(&worker_pid) {
        stop_pidfile(&worker_pid);
        log.push_str("starting avrag-worker\n");
        spawn_with_env(&worker_bin, &env_pairs, &worker_log, &worker_pid, None)?;
    }

    if wait_api_healthy(45, &mut log) {
        Ok(format!(
            "product API ready at {} (native spawn)\n{log}",
            client_api_base()
        ))
    } else {
        Err(format!(
            "API health timeout after spawn — see {}\n{log}",
            api_log.display()
        ))
    }
}

/// Stop product sidecars (API + worker). `pub(crate)` for exit lifecycle.
pub(crate) fn stop_product_native() -> String {
    let mut log = String::new();
    let (run_dir, _) = run_log_dir();
    // /T = kill process tree (Windows workers may spawn helpers).
    stop_pidfile_tree(&run_dir.join("api.pid"));
    stop_pidfile_tree(&run_dir.join("worker.pid"));
    log.push_str("stopped api/worker pidfiles (native)\n");
    log
}

fn stop_pidfile_tree(pidfile: &Path) {
    if let Ok(raw) = fs::read_to_string(pidfile) {
        let pid = raw.trim();
        if !pid.is_empty() {
            #[cfg(unix)]
            {
                let _ = Command::new("kill")
                    .arg(pid)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
                thread::sleep(Duration::from_millis(200));
                let _ = Command::new("kill")
                    .args(["-9", pid])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
            #[cfg(windows)]
            {
                if let Ok(p) = pid.parse::<u32>() {
                    let _ = super::win_cmd::kill_pid_tree(p);
                }
            }
        }
    }
    let _ = fs::remove_file(pidfile);
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
    let mut bash = Command::new("bash");
    bash.arg(&script)
        .arg(arg)
        .current_dir(&root)
        .env("CONTEXT_OS_ROOT", root.as_os_str());
    super::win_cmd::hide_console(&mut bash);
    let output = bash.output()
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
    // Fast path.
    let early = build_status();
    if early.api_ok {
        return Ok(EnsureLocalProductResult {
            ok: true,
            message: format!("Local product API already ready at {}.", early.api_base_url),
            stdout: String::new(),
            stderr: String::new(),
            status: early,
        });
    }

    // 1) Pure-Rust native spawn (install + monorepo).
    let native = tokio::task::spawn_blocking(ensure_product_native)
        .await
        .map_err(|e| IpcApiError::internal(format!("ensure product native join: {e}")))?;

    match native {
        Ok(msg) => {
            let status = build_status();
            if status.api_ok {
                return Ok(EnsureLocalProductResult {
                    ok: true,
                    message: msg.lines().next().unwrap_or("product ready").to_string(),
                    stdout: msg,
                    stderr: String::new(),
                    status,
                });
            }
        }
        Err(native_err) => {
            // 2) Bash monorepo fallback.
            if monorepo_root().is_some() {
                let script_result = tokio::task::spawn_blocking(|| run_product_script("ensure"))
                    .await
                    .map_err(|e| IpcApiError::internal(format!("ensure product join: {e}")))?;
                if let Ok((code, stdout, stderr)) = script_result {
                    let status = build_status();
                    let ok = code == 0 && status.api_ok;
                    let message = if ok {
                        format!(
                            "Local product API ready at {} (bash; native first: {native_err}).",
                            status.api_base_url
                        )
                    } else {
                        format!(
                            "native: {native_err}; bash ensure exit {code}: {}",
                            stderr.lines().last().unwrap_or("see stderr")
                        )
                    };
                    return Ok(EnsureLocalProductResult {
                        ok,
                        message,
                        stdout: format!("--- native err ---\n{native_err}\n--- bash ---\n{stdout}"),
                        stderr,
                        status,
                    });
                }
            }

            let status = build_status();
            return Ok(EnsureLocalProductResult {
                ok: false,
                message: format!(
                    "本机产品进程启动失败：{native_err}。请确认 avrag-api/worker 已打包，且数据栈已就绪。"
                ),
                stdout: String::new(),
                stderr: native_err,
                status,
            });
        }
    }

    // Native returned Ok message but health still false — try bash if available.
    if monorepo_root().is_some() {
        if let Ok((code, stdout, stderr)) = tokio::task::spawn_blocking(|| run_product_script("ensure"))
            .await
            .map_err(|e| IpcApiError::internal(format!("ensure product join: {e}")))?
        {
            let status = build_status();
            let ok = code == 0 && status.api_ok;
            return Ok(EnsureLocalProductResult {
                ok,
                message: if ok {
                    format!("Local product API ready at {}.", status.api_base_url)
                } else {
                    format!(
                        "product ensure incomplete (exit {code}). {}",
                        stderr.lines().last().unwrap_or("see logs")
                    )
                },
                stdout,
                stderr,
                status,
            });
        }
    }

    let status = build_status();
    Ok(EnsureLocalProductResult {
        ok: status.api_ok,
        message: if status.api_ok {
            format!("Local product API ready at {}.", status.api_base_url)
        } else {
            format!(
                "产品进程未就绪：{}。日志目录：{}",
                status.health_detail,
                status.log_dir.as_deref().unwrap_or("(unknown)")
            )
        },
        stdout: String::new(),
        stderr: String::new(),
        status,
    })
}

#[tauri::command]
pub async fn stop_local_product() -> Result<EnsureLocalProductResult, IpcApiError> {
    let native_log = tokio::task::spawn_blocking(stop_product_native)
        .await
        .map_err(|e| IpcApiError::internal(format!("stop product join: {e}")))?;

    let mut stdout = format!("--- native ---\n{native_log}\n");
    let mut stderr = String::new();
    if monorepo_root().is_some() {
        if let Ok((code, out, err)) = tokio::task::spawn_blocking(|| run_product_script("stop"))
            .await
            .map_err(|e| IpcApiError::internal(format!("stop product script join: {e}")))?
        {
            stdout.push_str("--- bash ---\n");
            stdout.push_str(&out);
            stderr = err;
            let _ = code;
        }
    }

    let status = build_status();
    let ok = !status.api_ok;
    Ok(EnsureLocalProductResult {
        ok,
        message: if ok {
            "Local product API/worker stopped.".into()
        } else {
            "stop 已调用，但 API 端口仍在监听。".into()
        },
        stdout,
        stderr,
        status,
    })
}

/// Force restart the local product (api + worker) so newly upserted provider
/// secrets (BYOK embedding/rerank) are resolved at bootstrap. Bypasses the
/// `ensure_local_product` "already healthy" fast path by stopping first.
#[tauri::command]
pub async fn restart_local_product() -> Result<EnsureLocalProductResult, IpcApiError> {
    let stop_log = tokio::task::spawn_blocking(stop_product_native)
        .await
        .map_err(|e| IpcApiError::internal(format!("stop product join: {e}")))?;
    let mut stdout = format!("--- stop ---\n{stop_log}\n");
    // Give the OS a beat to release the listen port before re-ensure.
    tokio::time::sleep(Duration::from_secs(2)).await;

    let mut result = ensure_local_product().await?;
    result.stdout = format!("{stdout}--- ensure ---\n{}", result.stdout);
    Ok(result)
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

    #[test]
    fn parse_env_skips_comments() {
        let dir = std::env::temp_dir().join(format!("cos-env-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("client.env");
        fs::write(
            &path,
            "# comment\nJWT_SECRET=abc\nAVRAG_PUBLIC_BASE_URL=http://127.0.0.1:18080\n",
        )
        .unwrap();
        let pairs = parse_env_file(&path);
        assert!(pairs.iter().any(|(k, v)| k == "JWT_SECRET" && v == "abc"));
        let _ = fs::remove_dir_all(&dir);
    }
}
