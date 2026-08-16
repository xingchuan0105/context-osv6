//! Closed-app lifecycle: tear down every local component when the shell exits.
//!
//! Best practice for a self-contained desktop product (Tauri + portable data plane):
//! 1. **Product first** — stop API/worker so they release PG/Redis connections.
//! 2. **Graceful data plane** — `pg_ctl stop -m fast` (Postgres official fast shutdown);
//!    Redis via pidfile / scoped kill (Windows Redis often has no clean daemon stop).
//! 3. **Scoped sweep** — force-kill only processes whose *executable path* is under
//!    this install / state tree (never system-wide `postgres.exe` / other users' Redis).
//! 4. Hook **Tauri `RunEvent::Exit`** so close-window, tray quit, and process exit share one path.
//!
//! Refs: Postgres shutdown modes; Tauri sidecar cleanup on Exit; portable app process trees.

use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::Command;

use super::local_product;
use super::native_stack;

/// Full local runtime teardown (product + PG + Redis + scoped leftovers).
/// Safe to call multiple times; errors are absorbed into the returned log.
pub fn shutdown_all_local_runtime() -> String {
    let mut log = String::new();
    log.push_str("=== lifecycle shutdown begin ===\n");

    // 1) Product sidecars (avrag-api / avrag-worker)
    let product = local_product::stop_product_native();
    log.push_str("--- product ---\n");
    log.push_str(&product);
    if !product.ends_with('\n') {
        log.push('\n');
    }

    // 2) Data plane (Postgres fast + Redis)
    let stack = native_stack::stop_native();
    log.push_str("--- native stack ---\n");
    log.push_str(&stack.log);
    if !stack.log.ends_with('\n') {
        log.push('\n');
    }

    // 3) Scoped force-kill under install / state roots (orphans, failed pidfiles)
    let sweep = sweep_scoped_children();
    log.push_str("--- scoped sweep ---\n");
    log.push_str(&sweep);
    if !sweep.ends_with('\n') {
        log.push('\n');
    }

    log.push_str("=== lifecycle shutdown end ===\n");
    // Best-effort write for post-mortem (installer lock diagnosis).
    if let Some(rt) = native_stack::runtime_home() {
        let path = rt.join("logs").join("lifecycle-shutdown.log");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, &log);
    }
    log
}

fn install_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.to_path_buf());
        }
    }
    if let Some(rt) = native_stack::runtime_home() {
        roots.push(rt);
    }
    if let Some(bins) = native_stack::bins_runtime_home() {
        roots.push(bins);
    }
    // De-dup while preserving order
    let mut out = Vec::new();
    for r in roots {
        if r.as_os_str().is_empty() {
            continue;
        }
        if !out.iter().any(|x: &PathBuf| x == &r) {
            out.push(r);
        }
    }
    out
}

fn path_is_under(path: &Path, roots: &[PathBuf]) -> bool {
    let Ok(path) = path.canonicalize().or_else(|_| Ok::<_, ()>(path.to_path_buf())) else {
        return false;
    };
    for root in roots {
        let root = root.canonicalize().unwrap_or_else(|_| root.clone());
        if path.starts_with(&root) {
            return true;
        }
        // Windows path compare is case-insensitive; starts_with on Path is case-sensitive
        // on some platforms — fall back to lowercase string prefix.
        let ps = path.to_string_lossy().to_ascii_lowercase();
        let rs = root.to_string_lossy().to_ascii_lowercase();
        if !rs.is_empty() && ps.starts_with(&rs) {
            return true;
        }
    }
    false
}

/// Kill processes that still run from *our* install tree (api/worker/postgres/redis).
fn sweep_scoped_children() -> String {
    let roots = install_roots();
    if roots.is_empty() {
        return "no install roots — skip sweep\n".into();
    }
    let mut log = format!("roots={roots:?}\n");

    #[cfg(windows)]
    {
        // Win32 Toolhelp + TerminateProcess — no powershell/taskkill console flash.
        let lines = super::win_cmd::kill_named_under(
            &[
                "avrag-api",
                "avrag-worker",
                "postgres",
                "redis-server",
                "pg_ctl",
                "initdb",
            ],
            &roots,
        );
        if lines.is_empty() {
            log.push_str("no leftover scoped processes\n");
        } else {
            for line in lines {
                log.push_str(&line);
                log.push('\n');
            }
        }
    }

    #[cfg(unix)]
    {
        // Best-effort: pidfiles already handled; pkill when cmdline contains our root path.
        for root in &roots {
            let needle = root.display().to_string();
            if needle.len() < 12 {
                continue; // avoid overly broad pkill
            }
            // Single -f pattern: path fragment of our install/state tree.
            let _ = Command::new("pkill").args(["-f", &needle]).status();
            log.push_str(&format!("unix pkill -f under {needle}\n"));
        }
    }

    let _ = path_is_under;
    log
}
