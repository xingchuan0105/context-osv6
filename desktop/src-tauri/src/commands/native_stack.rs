//! Native (no Docker, no bash) data-plane control for desktop.
//!
//! Starts host `pg_ctl` + `redis-server` against `desktop/runtime/data/*-native`,
//! writes `client.env`, optionally runs `sqlx migrate`.
//! Falls back is handled by `local_stack` (bash script / docker).

use std::fs;
use std::net::{TcpStream, ToSocketAddrs};

use base64::Engine as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const PG_USER: &str = "avrag";
const PG_PASS: &str = "avrag";
const PG_DB: &str = "avrag_client";
const PG_PORT: u16 = 5433;
const REDIS_PORT: u16 = 6380;

#[derive(Debug, Clone)]
pub struct NativeEnsureReport {
    pub ok: bool,
    pub message: String,
    pub log: String,
}

fn append_log(log: &mut String, line: impl AsRef<str>) {
    log.push_str(line.as_ref());
    log.push('\n');
}

/// Process-global serializer for `ensure_native` (see its doc comment at the
/// call site) — concurrent ensure IPC calls must not interleave initdb /
/// createdb against the same cluster.
fn ensure_native_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    &LOCK
}

fn port_open(host: &str, port: u16) -> bool {
    let endpoint = format!("{host}:{port}");
    let Ok(addrs) = endpoint.to_socket_addrs() else {
        return false;
    };
    for a in addrs {
        if TcpStream::connect_timeout(&a, Duration::from_millis(300)).is_ok() {
            return true;
        }
    }
    false
}

fn wait_port(host: &str, port: u16, secs: u64, log: &mut String) -> bool {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < deadline {
        if port_open(host, port) {
            append_log(log, format!("port {host}:{port} OK"));
            return true;
        }
        thread::sleep(Duration::from_millis(400));
    }
    append_log(log, format!("timeout waiting for {host}:{port}"));
    false
}

/// True if this directory looks like a portable runtime (has pgsql bin tree).
fn looks_like_bins_runtime(dir: &Path) -> bool {
    pg_ctl_exists(&dir.join("pgsql/bin"))
        || pg_ctl_exists(&dir.join("bundled/windows-x64/pgsql/bin"))
        || pg_ctl_exists(&dir.join("bundled/linux-x64/pgsql/bin"))
        || pg_ctl_exists(&dir.join("native/pgsql/bin"))
}

/// Install layout: `$INSTDIR/runtime` or `$INSTDIR/resources/runtime` next to Context-OS.exe.
fn install_bins_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let candidates = [
        dir.join("runtime"),
        dir.join("resources").join("runtime"),
        dir.to_path_buf(),
    ];
    candidates.into_iter().find(|p| looks_like_bins_runtime(p))
}

/// Packaged client state under `%LOCALAPPDATA%\Context-OS Client` (not Program Files).
fn install_state_dir() -> Option<PathBuf> {
    if install_bins_dir().is_none() {
        return None;
    }
    if let Ok(p) = std::env::var("CONTEXT_OS_STATE_HOME") {
        let p = PathBuf::from(p);
        if !p.as_os_str().is_empty() {
            return Some(p);
        }
    }
    #[cfg(windows)]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            return Some(PathBuf::from(local).join("Context-OS Client"));
        }
    }
    #[cfg(not(windows))]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return Some(PathBuf::from(home).join(".local/share/context-os-client"));
        }
    }
    None
}

/// Monorepo `desktop/runtime` (dev).
fn monorepo_runtime_dir() -> Option<PathBuf> {
    if let Ok(root) = std::env::var("CONTEXT_OS_ROOT") {
        let p = PathBuf::from(root).join("desktop/runtime");
        if p.is_dir() {
            return Some(p);
        }
    }
    // src-tauri → desktop/runtime
    let mut m = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    m.pop(); // desktop
    let rt = m.join("runtime");
    if rt.is_dir() {
        return Some(rt);
    }
    // walk cwd
    if let Ok(mut cwd) = std::env::current_dir() {
        for _ in 0..8 {
            let cand = cwd.join("desktop/runtime");
            if cand.is_dir() {
                return Some(cand);
            }
            if cwd.join("runtime").join("client.env").exists() || cwd.join("runtime").is_dir() {
                let r = cwd.join("runtime");
                if r.is_dir() {
                    return Some(r);
                }
            }
            if !cwd.pop() {
                break;
            }
        }
    }
    None
}

/// Where portable **binaries** live (`pgsql/`, `redis/`). Install layout preferred.
pub fn bins_runtime_home() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("CONTEXT_OS_RUNTIME") {
        let p = PathBuf::from(home);
        if looks_like_bins_runtime(&p) || p.is_dir() {
            return Some(p);
        }
    }
    if let Some(d) = install_bins_dir() {
        return Some(d);
    }
    monorepo_runtime_dir()
}

/// Where **state** lives: `client.env`, `data/`, `logs/`.
/// Install: AppData; monorepo: `desktop/runtime`.
pub fn runtime_home() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("CONTEXT_OS_CLIENT_HOME") {
        let p = PathBuf::from(home);
        if p.is_dir() || p.parent().map(|x| x.is_dir()).unwrap_or(false) {
            return Some(p);
        }
    }
    if let Some(state) = install_state_dir() {
        return Some(state);
    }
    monorepo_runtime_dir()
}

pub fn monorepo_root_from_runtime(rt: &Path) -> Option<PathBuf> {
    // .../desktop/runtime → repo root
    let mut p = rt.to_path_buf();
    if p.file_name().and_then(|s| s.to_str()) == Some("runtime") {
        p.pop();
        if p.file_name().and_then(|s| s.to_str()) == Some("desktop") {
            p.pop();
            if p.join("avrag-rs/migrations").is_dir() {
                return Some(p);
            }
        }
    }
    std::env::var("CONTEXT_OS_ROOT").ok().map(PathBuf::from)
}

fn resolve_migrations_dir(bins_rt: &Path, state_rt: &Path) -> Option<PathBuf> {
    for cand in [
        bins_rt.join("migrations"),
        state_rt.join("migrations"),
        bins_rt.join("bundled/windows-x64/migrations"),
    ] {
        if cand.is_dir() {
            return Some(cand);
        }
    }
    monorepo_root_from_runtime(state_rt)
        .or_else(|| monorepo_root_from_runtime(bins_rt))
        .map(|r| r.join("avrag-rs/migrations"))
        .filter(|p| p.is_dir())
}

fn pg_ctl_exists(dir: &Path) -> bool {
    dir.join("pg_ctl").is_file() || dir.join("pg_ctl.exe").is_file()
}

fn redis_server_exists(path: &Path) -> bool {
    path.is_file()
}

/// Prefer install/bundled portable trees over system packages (unless COS_USE_SYSTEM_PG=1).
/// Order: env → install runtime/pgsql → monorepo bundled/* → native/ → system → PATH.
fn bundled_pg_bin_candidates(rt: &Path) -> Vec<PathBuf> {
    vec![
        // Install layout (NSIS / BR2): $INSTDIR/runtime/pgsql/bin
        rt.join("pgsql/bin"),
        // Monorepo stage (BR1)
        rt.join("bundled/windows-x64/pgsql/bin"),
        rt.join("bundled/linux-x64/pgsql/bin"),
        // Host triple folder if ever used
        rt.join(format!("bundled/{}/pgsql/bin", std::env::consts::OS)),
        // Dev manual stage
        rt.join("native/pgsql/bin"),
    ]
}

fn bundled_redis_candidates(rt: &Path) -> Vec<PathBuf> {
    vec![
        rt.join("redis/redis-server.exe"),
        rt.join("redis/redis-server"),
        rt.join("bundled/windows-x64/redis/redis-server.exe"),
        rt.join("bundled/windows-x64/redis/redis-server"),
        rt.join("bundled/linux-x64/redis/redis-server"),
        rt.join("bundled/linux-x64/redis/redis-server.exe"),
        rt.join("native/redis/redis-server"),
        rt.join("native/redis/redis-server.exe"),
    ]
}

fn use_system_pg_only() -> bool {
    matches!(
        std::env::var("COS_USE_SYSTEM_PG").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes")
    )
}

fn find_pg_bin() -> Option<PathBuf> {
    if let Ok(d) = std::env::var("PG_BIN_DIR") {
        let p = PathBuf::from(d);
        if pg_ctl_exists(&p) {
            return Some(p);
        }
    }

    if !use_system_pg_only() {
        if let Some(rt) = bins_runtime_home() {
            for cand in bundled_pg_bin_candidates(&rt) {
                if pg_ctl_exists(&cand) {
                    return Some(cand);
                }
            }
        }
    }

    let system = [
        "/usr/lib/postgresql/16/bin",
        "/usr/lib/postgresql/15/bin",
        "/usr/lib/postgresql/17/bin",
        "/usr/local/pgsql/bin",
    ];
    for c in system {
        let p = PathBuf::from(c);
        if p.join("pg_ctl").is_file() {
            return Some(p);
        }
    }
    which("pg_ctl").and_then(|p| p.parent().map(|d| d.to_path_buf()))
}

fn find_redis_server() -> Option<PathBuf> {
    if let Ok(b) = std::env::var("REDIS_SERVER_BIN") {
        let p = PathBuf::from(b);
        if redis_server_exists(&p) {
            return Some(p);
        }
    }

    if !use_system_pg_only() {
        if let Some(rt) = bins_runtime_home() {
            for cand in bundled_redis_candidates(&rt) {
                if redis_server_exists(&cand) {
                    return Some(cand);
                }
            }
        }
    }

    which("redis-server").or_else(|| which("redis-server.exe"))
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for cand in [dir.join(name), dir.join(format!("{name}.exe"))] {
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

fn bin(pg_bin: &Path, name: &str) -> PathBuf {
    let unix = pg_bin.join(name);
    if unix.exists() {
        return unix;
    }
    pg_bin.join(format!("{name}.exe"))
}

fn apply_windows_no_window(cmd: &mut Command) {
    super::win_cmd::hide_console(cmd);
}

fn apply_windows_detached(cmd: &mut Command) {
    super::win_cmd::hide_and_detach(cmd);
}

fn run_capture(cmd: &mut Command) -> (i32, String, String) {
    apply_windows_no_window(cmd);
    match cmd.output() {
        Ok(o) => (
            o.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&o.stdout).to_string(),
            String::from_utf8_lossy(&o.stderr).to_string(),
        ),
        Err(e) => (-1, String::new(), e.to_string()),
    }
}

/// Run a command without capturing pipes (avoids Windows hangs when tools like
/// `pg_ctl -w` block on full stdout pipes under CREATE_NO_WINDOW).
fn run_status_null(cmd: &mut Command) -> i32 {
    apply_windows_no_window(cmd);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    match cmd.status() {
        Ok(s) => s.code().unwrap_or(-1),
        Err(_) => -1,
    }
}

fn flush_ensure_log(state_rt: &Path, log: &str) {
    let path = state_rt.join("logs").join("ensure-native.log");
    if let Some(p) = path.parent() {
        let _ = fs::create_dir_all(p);
    }
    let _ = fs::write(path, log);
}

/// Deterministic local account identity derived from the machine id.
/// Personal B2C model: owner == user (one `users` row IS the account), so both
/// ids must be the SAME uuid — `user_provider_secrets.owner_user_id` FK → `users.id`.
fn local_identity_uuids() -> (String, String) {
    let device_id = crate::commands::license::compute_device_id()
        .unwrap_or_else(|_| "cos-local-device".to_string());
    let id = uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_OID,
        format!("cos-local:{device_id}").as_bytes(),
    );
    (id.to_string(), id.to_string())
}

fn write_client_env(rt: &Path, migrations: Option<&Path>, log: &mut String) -> Result<(), String> {
    let env_path = rt.join("client.env");
    let jwt_path = rt.join("jwt.secret");
    let objects = rt.join("objects");
    fs::create_dir_all(&objects).map_err(|e| e.to_string())?;
    let jwt = if jwt_path.is_file() {
        fs::read_to_string(&jwt_path)
            .unwrap_or_default()
            .trim()
            .to_string()
    } else {
        let s = format!("{:x}", uuid::Uuid::new_v4().as_u128());
        fs::write(&jwt_path, format!("{s}\n")).map_err(|e| e.to_string())?;
        s
    };
    // BYOK envelope key for `user_provider_secrets` (ADR-0010 G1). Must be stable
    // across restarts: the local API decrypts previously upserted secrets with it.
    let byok_path = rt.join("byok.key");
    let byok = if byok_path.is_file() {
        fs::read_to_string(&byok_path)
            .unwrap_or_default()
            .trim()
            .to_string()
    } else {
        let a = uuid::Uuid::new_v4();
        let b = uuid::Uuid::new_v4();
        let mut bytes = [0u8; 32];
        bytes[..16].copy_from_slice(a.as_bytes());
        bytes[16..].copy_from_slice(b.as_bytes());
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        fs::write(&byok_path, format!("{encoded}\n")).map_err(|e| e.to_string())?;
        encoded
    };
    let (owner_id, user_id) = local_identity_uuids();
    let mig = migrations
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    // Bundled parser shims (install tree `runtime/parsers/`, dev tree
    // `desktop/runtime/parsers/`): stdlib-only parsers driven by the bundled
    // python, so text/office ingest works without a host-provided markitdown.
    // Windows-only: the shims are .cmd wrappers; other platforms keep the
    // ingestion defaults (PATH probing).
    let mut parsers_env = String::new();
    if cfg!(windows) {
        if let Some(bins_rt) = bins_runtime_home() {
            let dir = bins_rt.join("parsers");
            let md = dir.join("markitdown-lite.cmd");
            let ad = dir.join("anydoc-lite.cmd");
            // Cross-built liteparse CLI; pdfium.dll sits next to lit.exe and is
            // found by liteparse-pdfium-sys runtime search (exe dir rule).
            let lit = dir.join("lit").join("lit.exe");
            if md.is_file() {
                parsers_env.push_str(&format!("MARKITDOWN_BIN={}\n", md.display()));
            }
            if ad.is_file() {
                parsers_env.push_str(&format!("ANYDOC_BIN={}\n", ad.display()));
            }
            if lit.is_file() {
                parsers_env.push_str(&format!("LITEPARSE_BIN={}\n", lit.display()));
            }
        }
    }
    // Cloud relay credentials (W3): with a well-formed cloud session, platform
    // official models run through the cloud metered relay (`<cloud>/v1/relay`,
    // models pinned server-side) and the local api/worker must skip wallet
    // debit — the cloud already charged. No session → BYOK-only, unchanged.
    let mut relay_env = String::new();
    if let Some(session) = super::cloud_session::load_session_standalone() {
        let relay = &session.relay;
        relay_env = format!(
            "# 云登录官方模型（走余额）— cloud metered relay (W3)\n\
             AGENT_LLM_BASE_URL={relay_base}\n\
             AGENT_LLM_API_KEY={token}\n\
             AGENT_LLM_MODEL={chat_model}\n\
             EMBEDDING_BASE_URL={relay_base}\n\
             EMBEDDING_API_KEY={token}\n\
             EMBEDDING_MODEL={embedding_model}\n\
             INGESTION_LLM_BASE_URL={relay_base}\n\
             INGESTION_LLM_API_KEY={token}\n\
             INGESTION_LLM_MODEL={chat_model}\n\
             RERANK_BASE_URL={relay_base}\n\
             RERANK_API_KEY={token}\n\
             RERANK_MODEL={rerank_model}\n\
             AVRAG_PLATFORM_KEYS_RELAY=1\n",
            relay_base = relay.base_url,
            token = session.desktop_token,
            chat_model = relay.chat_model,
            embedding_model = relay.embedding_model,
            rerank_model = relay.rerank_model,
        );
    }
    let body = format!(
        r#"# Generated by desktop native_stack (no Docker / no bash)
STACK_MODE=native
CLIENT_PG_HOST=127.0.0.1
CLIENT_PG_PORT={PG_PORT}
CLIENT_REDIS_HOST=127.0.0.1
CLIENT_REDIS_PORT={REDIS_PORT}
CLIENT_API_HOST=127.0.0.1
CLIENT_API_PORT=18080
DATABASE_URL=postgres://{PG_USER}:{PG_PASS}@127.0.0.1:{PG_PORT}/{PG_DB}
REDIS_URL=redis://127.0.0.1:{REDIS_PORT}/0
REDIS_ADDR=127.0.0.1:{REDIS_PORT}
RETRIEVAL_BACKEND=pgvector
MILVUS_COLLECTION_PREFIX=avrag_client
AVRAG_API_ADDR=127.0.0.1:18080
AVRAG_PUBLIC_BASE_URL=http://127.0.0.1:18080
CORS_ALLOWED_ORIGINS=http://tauri.localhost,https://tauri.localhost,http://127.0.0.1:18080,http://localhost:18080,http://localhost:3000,http://127.0.0.1:3000
AVRAG_OBJECT_ROOT={objects}
JWT_SECRET={jwt}
BYOK_MASTER_KEY={byok}
NEXT_PUBLIC_DEV_OWNER_USER_ID={owner_id}
NEXT_PUBLIC_DEV_USER_ID={user_id}
AVRAG_RUN_MIGRATIONS=true
AVRAG_MIGRATIONS_DIR={mig}
# RAG availability is derived from whether an embedding client can be built
# (platform env or SiliconFlow purpose=embedding secret), not this gate.
AVRAG_ENABLE_RAG=true
# SiliconFlow BAAI/bge-m3 native output dim — schema sizing only, not a request field.
AVRAG_EMBEDDING_DIM=1024
{parsers_env}{relay_env}"#,
        objects = objects.display(),
        jwt = jwt,
        byok = byok,
        owner_id = owner_id,
        user_id = user_id,
        mig = mig,
        parsers_env = parsers_env,
        relay_env = relay_env,
    );
    fs::write(&env_path, body).map_err(|e| e.to_string())?;
    fs::write(rt.join("stack.mode"), "native\n").ok();
    append_log(log, format!("wrote {}", env_path.display()));
    Ok(())
}

fn ensure_postgres(pg_bin: &Path, pgdata: &Path, run_dir: &Path, log_file: &Path, log: &mut String) -> Result<(), String> {
    fs::create_dir_all(pgdata).map_err(|e| e.to_string())?;
    fs::create_dir_all(run_dir).map_err(|e| e.to_string())?;
    if let Some(parent) = log_file.parent() {
        fs::create_dir_all(parent).ok();
    }

    // Fast path: already listening (common after lifecycle restart race).
    if port_open("127.0.0.1", PG_PORT) {
        append_log(log, "postgres already listening");
        return Ok(());
    }

    let pg_ctl = bin(pg_bin, "pg_ctl");
    let initdb = bin(pg_bin, "initdb");
    let psql = bin(pg_bin, "psql");
    let createdb = bin(pg_bin, "createdb");

    // Stale postmaster.pid after crash blocks start — remove if no live port.
    let pid_marker = pgdata.join("postmaster.pid");
    if pid_marker.is_file() && !port_open("127.0.0.1", PG_PORT) {
        append_log(log, "removing stale postmaster.pid (port closed)");
        let _ = fs::remove_file(&pid_marker);
    }

    if !pgdata.join("PG_VERSION").is_file() {
        append_log(log, format!("initdb {}", pgdata.display()));
        // initdb can emit a lot of stdout; use null stdio on Windows to avoid pipe stalls.
        let mut init = Command::new(&initdb);
        init.arg("-D")
            .arg(pgdata)
            .arg("-U")
            .arg(PG_USER)
            .arg("--auth-local=trust")
            .arg("--auth-host=trust")
            .arg("--encoding=UTF8")
            .arg("--locale=C")
            .arg("-N");
        // Point portable PG at its own tree (share/timezonesets next to bin/..).
        if let Some(home) = pg_bin.parent() {
            init.env("PGROOT", home);
            init.env("PGSYSCONFDIR", home.join("etc"));
        }
        let code = run_status_null(&mut init);
        if code != 0 {
            // Fall back to capture for error text.
            let (c2, out, err) = run_capture(
                Command::new(&initdb)
                    .arg("-D")
                    .arg(pgdata)
                    .arg("-U")
                    .arg(PG_USER)
                    .arg("--auth-local=trust")
                    .arg("--auth-host=trust")
                    .arg("--encoding=UTF8")
                    .arg("--locale=C")
                    .arg("-N"),
            );
            append_log(log, out);
            append_log(log, err);
            return Err(format!("initdb failed code={c2}"));
        }
        let conf = pgdata.join("postgresql.conf");
        // Windows PG has no Unix sockets; never set unix_socket_directories there
        // (paths with spaces under "%LOCALAPPDATA%\Context-OS Client" also break -o).
        let extra = if cfg!(windows) {
            format!(
                "\nlisten_addresses = '127.0.0.1'\nport = {PG_PORT}\nmax_connections = 40\nshared_buffers = 128MB\ntimezone = 'UTC'\nlog_timezone = 'UTC'\n"
            )
        } else {
            format!(
                "\nlisten_addresses = '127.0.0.1'\nport = {PG_PORT}\nunix_socket_directories = '{}'\nmax_connections = 40\nshared_buffers = 128MB\n",
                run_dir.display()
            )
        };
        fs::OpenOptions::new()
            .append(true)
            .open(&conf)
            .and_then(|mut f| {
                use std::io::Write;
                f.write_all(extra.as_bytes())
            })
            .map_err(|e| e.to_string())?;
        let hba = format!(
            "# TYPE  DATABASE        USER            ADDRESS                 METHOD\n\
local   all             all                                     trust\n\
host    all             all             127.0.0.1/32            trust\n\
host    all             all             ::1/128                 trust\n"
        );
        fs::write(pgdata.join("pg_hba.conf"), hba).map_err(|e| e.to_string())?;
    }

    // Prefer TCP over `pg_ctl status` (another console exe flash).
    if !port_open("127.0.0.1", PG_PORT) {
        append_log(log, "starting postgres (direct, no pg_ctl)");
        // Spawn postgres.exe ourselves so CREATE_NO_WINDOW applies to the
        // postmaster. `pg_ctl start` launches postgres.exe *without* that flag
        // and Windows pops a console.
        let postgres = bin(pg_bin, "postgres");
        let log_out = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)
            .map_err(|e| format!("open postgres log: {e}"))?;
        let log_err = log_out.try_clone().map_err(|e| e.to_string())?;
        let mut start = Command::new(&postgres);
        start
            .arg("-D")
            .arg(pgdata)
            .arg("-p")
            .arg(PG_PORT.to_string())
            .arg("-c")
            .arg("listen_addresses=127.0.0.1")
            .stdin(Stdio::null())
            .stdout(Stdio::from(log_out))
            .stderr(Stdio::from(log_err));
        if let Some(home) = pg_bin.parent() {
            start.env("PGROOT", home);
        }
        #[cfg(not(windows))]
        {
            start.arg("-c").arg(format!("unix_socket_directories={}", run_dir.display()));
        }
        apply_windows_no_window(&mut start);
        let child = start
            .spawn()
            .map_err(|e| format!("spawn postgres: {e}"))?;
        append_log(log, format!("postgres spawned pid={}", child.id()));
        std::mem::forget(child);
        let _ = pg_ctl; // status unused on this path
    } else {
        append_log(log, "postgres already listening");
    }

    if !wait_port("127.0.0.1", PG_PORT, 45, log) {
        if let Ok(body) = fs::read_to_string(log_file) {
            let tail: String = body.lines().rev().take(12).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
            append_log(log, format!("postgres log tail:\n{tail}"));
        }
        return Err("postgres port not open".into());
    }

    // TCP-open ≠ SQL-ready: a fresh initdb accepts TCP before SQL. A second
    // ensure pass racing the first must not probe/createdb against a half-up
    // postgres (createdb fails there and the old code still latched the
    // .avrag_inited marker — every later pass then skipped createdb and the
    // API crashed on `database "avrag_client" does not exist`).
    let mut sql_ready = false;
    for _ in 0..60 {
        let (code, _, _) = run_capture(
            Command::new(&psql)
                .args(["-h", "127.0.0.1", "-p", &PG_PORT.to_string(), "-U", PG_USER, "-d", "postgres", "-tAc"])
                .arg("SELECT 1"),
        );
        if code == 0 {
            sql_ready = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    if !sql_ready {
        append_log(log, "postgres SQL not ready after TCP open");
        return Err("postgres SQL not ready after TCP open".into());
    }

    // Skip psql/createdb after first success — they are CONSOLE exes and flash on Windows.
    let inited = pgdata.join(".avrag_inited");
    if inited.is_file() {
        append_log(log, "skip psql/createdb (already initialized)");
        return Ok(());
    }

    // create db
    let (code, out, err) = run_capture(
        Command::new(&psql)
            .args(["-h", "127.0.0.1", "-p", &PG_PORT.to_string(), "-U", PG_USER, "-d", "postgres", "-tAc"])
            .arg(format!("SELECT 1 FROM pg_database WHERE datname='{PG_DB}'")),
    );
    append_log(log, format!("db exists probe code={code} {out}{err}"));
    if !out.contains('1') {
        let (c2, o2, e2) = run_capture(
            Command::new(&createdb)
                .args(["-h", "127.0.0.1", "-p", &PG_PORT.to_string(), "-U", PG_USER, PG_DB]),
        );
        append_log(log, format!("createdb {c2} {o2}{e2}"));
    }
    let _ = run_capture(
        Command::new(&psql)
            .args(["-h", "127.0.0.1", "-p", &PG_PORT.to_string(), "-U", PG_USER, "-d", PG_DB, "-c"])
            .arg(format!("ALTER USER {PG_USER} WITH PASSWORD '{PG_PASS}';")),
    );
    let _ = run_capture(
        Command::new(&psql)
            .args(["-h", "127.0.0.1", "-p", &PG_PORT.to_string(), "-U", PG_USER, "-d", PG_DB, "-c"])
            .arg("CREATE EXTENSION IF NOT EXISTS vector;"),
    );
    // Latch the marker only when the db verifiably exists — a failed createdb
    // must not mark the cluster initialized (next ensure then retries instead
    // of every later pass skipping createdb).
    let (_, verify, _) = run_capture(
        Command::new(&psql)
            .args(["-h", "127.0.0.1", "-p", &PG_PORT.to_string(), "-U", PG_USER, "-d", "postgres", "-tAc"])
            .arg(format!("SELECT 1 FROM pg_database WHERE datname='{PG_DB}'")),
    );
    if verify.trim() == "1" {
        let _ = fs::write(&inited, "1\n");
        Ok(())
    } else {
        append_log(log, format!("db {PG_DB} still missing after createdb step"));
        Err(format!("createdb {PG_DB} failed"))
    }
}

fn ensure_redis(redis_bin: &Path, data_dir: &Path, pid_file: &Path, log_file: &Path, log: &mut String) -> Result<(), String> {
    fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    if let Some(p) = log_file.parent() {
        fs::create_dir_all(p).ok();
    }
    if let Some(p) = pid_file.parent() {
        fs::create_dir_all(p).ok();
    }
    if port_open("127.0.0.1", REDIS_PORT) {
        append_log(log, "redis already listening");
        return Ok(());
    }
    append_log(log, "starting redis-server");
    // Windows Redis (tporadowski) does not fork on --daemonize; child.wait() would
    // block forever after the port is open and freeze cold-start (Postgres never starts).
    // Unix: daemonize + wait is fine. Windows: spawn detached and only wait on the port.
    let mut cmd = Command::new(redis_bin);
    #[cfg(not(windows))]
    {
        cmd.arg("--daemonize").arg("yes");
    }
    cmd.arg("--port")
        .arg(REDIS_PORT.to_string())
        .arg("--bind")
        .arg("127.0.0.1")
        .arg("--dir")
        .arg(data_dir)
        .arg("--dbfilename")
        .arg("dump.rdb")
        .arg("--appendonly")
        .arg("yes")
        .arg("--pidfile")
        .arg(pid_file)
        .arg("--logfile")
        .arg(log_file)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    apply_windows_detached(&mut cmd);

    let child = cmd
        .spawn()
        .map_err(|e| format!("spawn redis-server: {e}"))?;

    #[cfg(windows)]
    {
        let pid = child.id();
        let _ = fs::write(pid_file, format!("{pid}\n"));
        // Detach: do not wait — redis stays as a long-lived child of the session.
        std::mem::forget(child);
        append_log(log, format!("redis spawned pid={pid} (detached, no daemonize)"));
    }
    #[cfg(not(windows))]
    {
        let mut child = child;
        let _ = child.wait();
    }

    if !wait_port("127.0.0.1", REDIS_PORT, 15, log) {
        return Err("redis port not open".into());
    }
    Ok(())
}

fn run_migrate(migrations: &Path, log: &mut String) {
    let url = format!("postgres://{PG_USER}:{PG_PASS}@127.0.0.1:{PG_PORT}/{PG_DB}");
    let sqlx = which("sqlx").or_else(|| {
        let home = std::env::var_os("HOME")?;
        let p = PathBuf::from(home).join(".cargo/bin/sqlx");
        p.is_file().then_some(p)
    });
    let Some(sqlx) = sqlx else {
        append_log(log, "sqlx not found — skip migrate (install sqlx-cli)");
        return;
    };
    append_log(log, "sqlx migrate run");
    let (code, out, err) = run_capture(
        Command::new(sqlx)
            .arg("migrate")
            .arg("run")
            .arg("--source")
            .arg(migrations)
            .arg("--database-url")
            .arg(&url),
    );
    append_log(log, out);
    append_log(log, err);
    if code != 0 {
        append_log(
            log,
            "migrate non-zero (desktop soft-ok if rag_kg_* already present / pg_bigm missing)",
        );
    }
}

/// Returns true if native tools exist on PATH / known locations.
pub fn native_tools_available() -> bool {
    find_pg_bin().is_some() && find_redis_server().is_some()
}

/// Re-write client.env from the current state dir (cloud relay block included
/// when a cloud session exists). Called by cloud login/logout so an already
/// running local product can be restarted onto the new credentials without a
/// full stack ensure.
pub(crate) fn refresh_client_env() -> Result<String, String> {
    let mut log = String::new();
    let Some(state_rt) = runtime_home() else {
        return Err(
            "runtime state dir not found (set CONTEXT_OS_CLIENT_HOME or install/monorepo layout)"
                .into(),
        );
    };
    let bins_rt = bins_runtime_home().unwrap_or_else(|| state_rt.clone());
    let mig = resolve_migrations_dir(&bins_rt, &state_rt);
    write_client_env(&state_rt, mig.as_deref(), &mut log)?;
    flush_ensure_log(&state_rt, &log);
    Ok(log)
}

pub fn ensure_native() -> NativeEnsureReport {
    // Serialize concurrent ensures: bootstrap / product / login paths can race
    // (e.g. right after the W3 gate releases), and two passes running initdb
    // or createdb against the same half-initialized cluster fail in ways the
    // old code then latched (.avrag_inited). The second caller waits here and
    // takes the fast path once the first completes.
    let _ensure_guard = ensure_native_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut log = String::new();
    let Some(state_rt) = runtime_home() else {
        return NativeEnsureReport {
            ok: false,
            message: "runtime state dir not found (set CONTEXT_OS_CLIENT_HOME or install/monorepo layout)"
                .into(),
            log,
        };
    };
    let bins_rt = bins_runtime_home().unwrap_or_else(|| state_rt.clone());
    if let Err(e) = fs::create_dir_all(&state_rt) {
        return NativeEnsureReport {
            ok: false,
            message: format!("cannot create state dir {}: {e}", state_rt.display()),
            log,
        };
    }
    append_log(&mut log, format!("state={}", state_rt.display()));
    append_log(&mut log, format!("bins={}", bins_rt.display()));
    flush_ensure_log(&state_rt, &log);

    // Fast path: both ports open → only refresh client.env (no process spawn).
    if port_open("127.0.0.1", PG_PORT) && port_open("127.0.0.1", REDIS_PORT) {
        append_log(&mut log, "fast-path: pg+redis already up");
        let mig = resolve_migrations_dir(&bins_rt, &state_rt);
        if let Err(e) = write_client_env(&state_rt, mig.as_deref(), &mut log) {
            flush_ensure_log(&state_rt, &log);
            return NativeEnsureReport {
                ok: false,
                message: e,
                log,
            };
        }
        flush_ensure_log(&state_rt, &log);
        return NativeEnsureReport {
            ok: true,
            message: "native stack already up (ports open), client.env refreshed".into(),
            log,
        };
    }

    let Some(pg_bin) = find_pg_bin() else {
        flush_ensure_log(&state_rt, &log);
        return NativeEnsureReport {
            ok: false,
            message: "pg_ctl not found — stage bundled runtime (scripts/stage-desktop-bundled-runtime.sh fetch) or install PostgreSQL 16 + pgvector".into(),
            log,
        };
    };
    let Some(redis_bin) = find_redis_server() else {
        flush_ensure_log(&state_rt, &log);
        return NativeEnsureReport {
            ok: false,
            message: "redis-server not found — stage bundled runtime or install Redis".into(),
            log,
        };
    };
    append_log(&mut log, format!("pg_bin={}", pg_bin.display()));
    append_log(&mut log, format!("redis={}", redis_bin.display()));
    flush_ensure_log(&state_rt, &log);

    let pgdata = state_rt.join("data/pg-native");
    let redis_dir = state_rt.join("data/redis-native");
    let run_dir = state_rt.join("run");
    let logs = state_rt.join("logs");
    fs::create_dir_all(&logs).ok();
    fs::create_dir_all(&run_dir).ok();

    if let Err(e) = ensure_redis(
        &redis_bin,
        &redis_dir,
        &run_dir.join("redis-native.pid"),
        &logs.join("redis-native.log"),
        &mut log,
    ) {
        flush_ensure_log(&state_rt, &log);
        return NativeEnsureReport {
            ok: false,
            message: e,
            log,
        };
    }
    flush_ensure_log(&state_rt, &log);
    if let Err(e) = ensure_postgres(
        &pg_bin,
        &pgdata,
        &run_dir,
        &logs.join("postgres-native.log"),
        &mut log,
    ) {
        flush_ensure_log(&state_rt, &log);
        return NativeEnsureReport {
            ok: false,
            message: e,
            log,
        };
    }

    let mig = resolve_migrations_dir(&bins_rt, &state_rt);
    if let Err(e) = write_client_env(&state_rt, mig.as_deref(), &mut log) {
        flush_ensure_log(&state_rt, &log);
        return NativeEnsureReport {
            ok: false,
            message: e,
            log,
        };
    }
    // sqlx migrate is optional and can hang if a rogue sqlx is on PATH — skip on Windows install.
    #[cfg(not(windows))]
    if let Some(ref m) = mig {
        if m.is_dir() {
            run_migrate(m, &mut log);
        }
    }
    #[cfg(windows)]
    {
        let _ = mig;
        append_log(&mut log, "skip host sqlx migrate on Windows (API applies migrations)");
    }

    let ok = port_open("127.0.0.1", PG_PORT) && port_open("127.0.0.1", REDIS_PORT);
    flush_ensure_log(&state_rt, &log);
    NativeEnsureReport {
        ok,
        message: if ok {
            "native stack up (Postgres+pgvector + Redis), client.env written".into()
        } else {
            "native start finished but ports not both open".into()
        },
        log,
    }
}

pub fn stop_native() -> NativeEnsureReport {
    let mut log = String::new();
    let Some(state_rt) = runtime_home() else {
        return NativeEnsureReport {
            ok: true,
            message: "no runtime dir".into(),
            log,
        };
    };
    // Stop Postgres without `pg_ctl` on Windows (pg_ctl is a console exe and flashes).
    let pgdata = state_rt.join("data/pg-native");
    let postmaster = pgdata.join("postmaster.pid");
    if postmaster.is_file() {
        if let Ok(body) = fs::read_to_string(&postmaster) {
            if let Some(first) = body.lines().next() {
                if let Ok(pid) = first.trim().parse::<u32>() {
                    #[cfg(windows)]
                    {
                        let n = super::win_cmd::kill_pid_tree(pid);
                        append_log(&mut log, format!("postgres TerminateProcess pid={pid} n={n}"));
                    }
                    #[cfg(not(windows))]
                    {
                        let _ = Command::new("kill").args(["-INT", &pid.to_string()]).status();
                        std::thread::sleep(Duration::from_millis(400));
                        if port_open("127.0.0.1", PG_PORT) {
                            let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
                        }
                        append_log(&mut log, format!("postgres kill pid={pid}"));
                    }
                }
            }
        }
    } else {
        #[cfg(windows)]
        {
            let n = super::win_cmd::kill_named_under(
                &["postgres", "pg_ctl"],
                &[state_rt.clone()],
            );
            append_log(&mut log, format!("postgres pidfile missing; scoped kill {n:?}"));
        }
        #[cfg(not(windows))]
        if let Some(pg_bin) = find_pg_bin() {
            if pgdata.join("PG_VERSION").is_file() {
                let code = run_status_null(
                    Command::new(bin(&pg_bin, "pg_ctl"))
                        .arg("-D")
                        .arg(&pgdata)
                        .arg("-m")
                        .arg("fast")
                        .arg("stop"),
                );
                append_log(&mut log, format!("pg_ctl stop -m fast {code}"));
            }
        }
    }
    // Redis: pidfile first; Windows Redis rarely supports graceful SHUTDOWN from CLI without redis-cli.
    let pid_file = state_rt.join("run/redis-native.pid");
    if pid_file.is_file() {
        if let Ok(pid_s) = fs::read_to_string(&pid_file) {
            if let Ok(pid) = pid_s.trim().parse::<i32>() {
                #[cfg(windows)]
                {
                    let n = super::win_cmd::kill_pid_tree(pid as u32);
                    append_log(&mut log, format!("redis TerminateProcess tree n={n}"));
                }
                #[cfg(not(windows))]
                {
                    let _ = Command::new("kill").arg(pid.to_string()).status();
                    std::thread::sleep(Duration::from_millis(150));
                    let _ = Command::new("kill")
                        .args(["-9", &pid.to_string()])
                        .status();
                }
                append_log(&mut log, format!("stopped redis pid {pid}"));
            }
        }
        let _ = fs::remove_file(&pid_file);
    } else if port_open("127.0.0.1", REDIS_PORT) {
        // Orphan redis on our port — try redis-cli SHUTDOWN if present next to server.
        if let Some(redis_bin) = find_redis_server() {
            if let Some(dir) = redis_bin.parent() {
                let cli = bin(dir, "redis-cli");
                if cli.is_file() {
                    let (c, o, e) = run_capture(
                        Command::new(&cli)
                            .args(["-p", &REDIS_PORT.to_string(), "SHUTDOWN", "NOSAVE"]),
                    );
                    append_log(&mut log, format!("redis-cli SHUTDOWN {c} {o}{e}"));
                }
            }
        }
    }
    NativeEnsureReport {
        ok: true,
        message: "native stack stop attempted".into(),
        log,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn which_finds_something_or_not() {
        // smoke: function does not panic
        let _ = find_pg_bin();
        let _ = find_redis_server();
        let _ = native_tools_available();
    }

    #[test]
    fn bundled_candidates_prefer_install_then_stage() {
        let rt = PathBuf::from("/tmp/fake-runtime-home");
        let pg = bundled_pg_bin_candidates(&rt);
        assert!(pg[0].ends_with("pgsql/bin"));
        assert!(pg.iter().any(|p| p.to_string_lossy().contains("bundled/windows-x64")));
        let redis = bundled_redis_candidates(&rt);
        assert!(redis[0].ends_with("redis/redis-server.exe") || redis[0].ends_with("redis/redis-server"));
    }
}
