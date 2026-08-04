//! Native (no Docker, no bash) data-plane control for desktop.
//!
//! Starts host `pg_ctl` + `redis-server` against `desktop/runtime/data/*-native`,
//! writes `client.env`, optionally runs `sqlx migrate`.
//! Falls back is handled by `local_stack` (bash script / docker).

use std::fs;
use std::net::{TcpStream, ToSocketAddrs};
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

/// Monorepo root or install layout that contains `desktop/runtime`.
pub fn runtime_home() -> Option<PathBuf> {
    if let Ok(root) = std::env::var("CONTEXT_OS_ROOT") {
        let p = PathBuf::from(root).join("desktop/runtime");
        if p.is_dir() {
            return Some(p);
        }
    }
    if let Ok(home) = std::env::var("CONTEXT_OS_CLIENT_HOME") {
        let p = PathBuf::from(home);
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
        if let Some(rt) = runtime_home() {
            for cand in bundled_pg_bin_candidates(&rt) {
                if pg_ctl_exists(&cand) {
                    return Some(cand);
                }
            }
        }
        if let Ok(extra) = std::env::var("CONTEXT_OS_RUNTIME") {
            let p = PathBuf::from(extra).join("pgsql/bin");
            if pg_ctl_exists(&p) {
                return Some(p);
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
        if let Some(rt) = runtime_home() {
            for cand in bundled_redis_candidates(&rt) {
                if redis_server_exists(&cand) {
                    return Some(cand);
                }
            }
        }
        if let Ok(extra) = std::env::var("CONTEXT_OS_RUNTIME") {
            for name in ["redis-server.exe", "redis-server"] {
                let p = PathBuf::from(&extra).join("redis").join(name);
                if redis_server_exists(&p) {
                    return Some(p);
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

fn run_capture(cmd: &mut Command) -> (i32, String, String) {
    match cmd.output() {
        Ok(o) => (
            o.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&o.stdout).to_string(),
            String::from_utf8_lossy(&o.stderr).to_string(),
        ),
        Err(e) => (-1, String::new(), e.to_string()),
    }
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
    let mig = migrations
        .map(|p| p.display().to_string())
        .unwrap_or_default();
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
AVRAG_OBJECT_ROOT={objects}
JWT_SECRET={jwt}
AVRAG_RUN_MIGRATIONS=true
AVRAG_MIGRATIONS_DIR={mig}
"#,
        objects = objects.display(),
        jwt = jwt,
        mig = mig,
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

    let pg_ctl = bin(pg_bin, "pg_ctl");
    let initdb = bin(pg_bin, "initdb");
    let psql = bin(pg_bin, "psql");
    let createdb = bin(pg_bin, "createdb");

    if !pgdata.join("PG_VERSION").is_file() {
        append_log(log, format!("initdb {}", pgdata.display()));
        let (code, out, err) = run_capture(
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
        if code != 0 {
            return Err(format!("initdb failed code={code}"));
        }
        let conf = pgdata.join("postgresql.conf");
        let extra = format!(
            "\nlisten_addresses = '127.0.0.1'\nport = {PG_PORT}\nunix_socket_directories = '{}'\nmax_connections = 40\nshared_buffers = 128MB\n",
            run_dir.display()
        );
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

    // status
    let (st, _, _) = run_capture(Command::new(&pg_ctl).arg("-D").arg(pgdata).arg("status"));
    if st != 0 {
        append_log(log, "pg_ctl start");
        let (code, out, err) = run_capture(
            Command::new(&pg_ctl)
                .arg("-D")
                .arg(pgdata)
                .arg("-l")
                .arg(log_file)
                .arg("-w")
                .arg("start")
                .arg("-o")
                .arg(format!(
                    "-p {PG_PORT} -c listen_addresses=127.0.0.1 -c unix_socket_directories={}",
                    run_dir.display()
                )),
        );
        append_log(log, out);
        append_log(log, err);
        if code != 0 {
            return Err(format!("pg_ctl start failed code={code}"));
        }
    } else {
        append_log(log, "postgres already running");
    }

    if !wait_port("127.0.0.1", PG_PORT, 30, log) {
        return Err("postgres port not open".into());
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
    Ok(())
}

fn ensure_redis(redis_bin: &Path, data_dir: &Path, pid_file: &Path, log_file: &Path, log: &mut String) -> Result<(), String> {
    fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    if let Some(p) = log_file.parent() {
        fs::create_dir_all(p).ok();
    }
    if port_open("127.0.0.1", REDIS_PORT) {
        append_log(log, "redis already listening");
        return Ok(());
    }
    append_log(log, "starting redis-server");
    let mut child = Command::new(redis_bin)
        .arg("--daemonize")
        .arg("yes")
        .arg("--port")
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
        .spawn()
        .map_err(|e| format!("spawn redis-server: {e}"))?;
    let _ = child.wait();
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

pub fn ensure_native() -> NativeEnsureReport {
    let mut log = String::new();
    let Some(rt) = runtime_home() else {
        return NativeEnsureReport {
            ok: false,
            message: "desktop/runtime not found (set CONTEXT_OS_ROOT or CONTEXT_OS_CLIENT_HOME)"
                .into(),
            log,
        };
    };
    append_log(&mut log, format!("runtime={}", rt.display()));

    let Some(pg_bin) = find_pg_bin() else {
        return NativeEnsureReport {
            ok: false,
            message: "pg_ctl not found — stage bundled runtime (scripts/stage-desktop-bundled-runtime.sh fetch) or install PostgreSQL 16 + pgvector".into(),
            log,
        };
    };
    let Some(redis_bin) = find_redis_server() else {
        return NativeEnsureReport {
            ok: false,
            message: "redis-server not found — stage bundled runtime or install Redis".into(),
            log,
        };
    };
    append_log(&mut log, format!("pg_bin={}", pg_bin.display()));
    append_log(&mut log, format!("redis={}", redis_bin.display()));

    let pgdata = rt.join("data/pg-native");
    let redis_dir = rt.join("data/redis-native");
    let run_dir = rt.join("run");
    let logs = rt.join("logs");
    fs::create_dir_all(&logs).ok();

    if let Err(e) = ensure_redis(
        &redis_bin,
        &redis_dir,
        &run_dir.join("redis-native.pid"),
        &logs.join("redis-native.log"),
        &mut log,
    ) {
        return NativeEnsureReport {
            ok: false,
            message: e,
            log,
        };
    }
    if let Err(e) = ensure_postgres(
        &pg_bin,
        &pgdata,
        &run_dir,
        &logs.join("postgres-native.log"),
        &mut log,
    ) {
        return NativeEnsureReport {
            ok: false,
            message: e,
            log,
        };
    }

    let mig = monorepo_root_from_runtime(&rt).map(|r| r.join("avrag-rs/migrations"));
    if let Err(e) = write_client_env(&rt, mig.as_deref(), &mut log) {
        return NativeEnsureReport {
            ok: false,
            message: e,
            log,
        };
    }
    if let Some(ref m) = mig {
        if m.is_dir() {
            run_migrate(m, &mut log);
        }
    }

    let ok = port_open("127.0.0.1", PG_PORT) && port_open("127.0.0.1", REDIS_PORT);
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
    let Some(rt) = runtime_home() else {
        return NativeEnsureReport {
            ok: true,
            message: "no runtime dir".into(),
            log,
        };
    };
    let pgdata = rt.join("data/pg-native");
    if let Some(pg_bin) = find_pg_bin() {
        if pgdata.join("PG_VERSION").is_file() {
            let (c, o, e) = run_capture(
                Command::new(bin(&pg_bin, "pg_ctl"))
                    .arg("-D")
                    .arg(&pgdata)
                    .arg("-m")
                    .arg("fast")
                    .arg("-w")
                    .arg("stop"),
            );
            append_log(&mut log, format!("pg_ctl stop {c} {o}{e}"));
        }
    }
    let pid_file = rt.join("run/redis-native.pid");
    if pid_file.is_file() {
        if let Ok(pid_s) = fs::read_to_string(&pid_file) {
            if let Ok(pid) = pid_s.trim().parse::<i32>() {
                let _ = Command::new("kill").arg(pid.to_string()).status();
                append_log(&mut log, format!("killed redis pid {pid}"));
            }
        }
        let _ = fs::remove_file(&pid_file);
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
