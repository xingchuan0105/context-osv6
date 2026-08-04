//! Docker / Docker Desktop availability probe + install guidance.

use serde::Serialize;
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct DockerStatus {
    /// `docker` CLI is on PATH.
    pub cli_ok: bool,
    /// Daemon responds (`docker info`).
    pub daemon_ok: bool,
    /// `docker compose version` works.
    pub compose_ok: bool,
    pub overall_ok: bool,
    pub detail: String,
    /// Platform-specific download / docs URL.
    pub install_url: String,
    /// Short human-readable install guide (zh).
    pub install_hint: String,
    pub platform: String,
}

fn run_cmd(program: &str, args: &[&str], timeout_secs: u64) -> (bool, String) {
    // Best-effort: spawn and wait with a soft timeout via thread + kill is heavy;
    // docker info usually returns quickly. Use status only.
    let _ = timeout_secs;
    match Command::new(program).args(args).output() {
        Ok(out) => {
            let ok = out.status.success();
            let mut msg = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if msg.is_empty() {
                msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
            }
            if msg.len() > 240 {
                msg.truncate(240);
                msg.push('…');
            }
            (ok, msg)
        }
        Err(e) => (false, e.to_string()),
    }
}

fn platform_label() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

fn install_meta() -> (String, String) {
    // Desktop default is **native** PG+Redis (no Docker). Docker is optional fallback only.
    match platform_label() {
        "windows" => (
            "https://www.postgresql.org/download/windows/".into(),
            "默认不使用 Docker。请安装 PostgreSQL 16 + pgvector 扩展，以及 Redis（或 Memurai），\
             保证 pg_ctl / redis-server 在 PATH；然后点「启动并迁移」。仅当无法装本机 PG 时才考虑 Docker Desktop。"
                .into(),
        ),
        "macos" => (
            "https://www.postgresql.org/download/macosx/".into(),
            "默认不使用 Docker。可用 Homebrew：brew install postgresql@16 redis，并安装 pgvector；\
             然后点「启动并迁移」。Docker 仅作可选回退。"
                .into(),
        ),
        _ => (
            "https://www.postgresql.org/download/linux/".into(),
            "默认不使用 Docker。Debian/Ubuntu：sudo apt-get install -y postgresql-16 \
             postgresql-16-pgvector redis-server；然后点「启动并迁移」。Docker 仅作可选回退。"
                .into(),
        ),
    }
}

fn build_status() -> DockerStatus {
    let (install_url, install_hint) = install_meta();
    let platform = platform_label().to_string();

    let (cli_ok, cli_detail) = run_cmd("docker", &["version", "--format", "{{.Client.Version}}"], 5);
    if !cli_ok {
        // Windows may need docker.exe explicit — try once more.
        let (cli2, d2) = run_cmd("docker.exe", &["version", "--format", "{{.Client.Version}}"], 5);
        if !cli2 {
            return DockerStatus {
                cli_ok: false,
                daemon_ok: false,
                compose_ok: false,
                overall_ok: false,
                detail: format!("未检测到 Docker CLI（{cli_detail}）"),
                install_url,
                install_hint,
                platform,
            };
        }
        return finish_probe(true, d2, true, install_url, install_hint, platform);
    }

    finish_probe(cli_ok, cli_detail, false, install_url, install_hint, platform)
}

fn finish_probe(
    cli_ok: bool,
    cli_detail: String,
    use_exe: bool,
    install_url: String,
    install_hint: String,
    platform: String,
) -> DockerStatus {
    let docker = if use_exe { "docker.exe" } else { "docker" };
    let (daemon_ok, daemon_detail) = run_cmd(docker, &["info", "--format", "{{.ServerVersion}}"], 8);
    let (compose_ok, compose_detail) = {
        let (a, da) = run_cmd(docker, &["compose", "version"], 5);
        if a {
            (true, da)
        } else {
            // Legacy docker-compose binary
            let (b, db) = run_cmd("docker-compose", &["version"], 5);
            (b, db)
        }
    };

    let overall_ok = cli_ok && daemon_ok && compose_ok;
    let detail = if overall_ok {
        format!(
            "Docker OK · client {} · engine {} · compose {}",
            cli_detail.lines().next().unwrap_or("ok"),
            daemon_detail.lines().next().unwrap_or("ok"),
            compose_detail.lines().next().unwrap_or("ok")
        )
    } else if !cli_ok {
        format!("CLI 不可用: {cli_detail}")
    } else if !daemon_ok {
        format!(
            "CLI 已安装，但引擎未就绪（请启动 Docker Desktop / dockerd）: {daemon_detail}"
        )
    } else {
        format!("docker compose 不可用: {compose_detail}")
    };

    DockerStatus {
        cli_ok,
        daemon_ok,
        compose_ok,
        overall_ok,
        detail,
        install_url,
        install_hint,
        platform,
    }
}

#[tauri::command]
pub fn get_docker_status() -> DockerStatus {
    build_status()
}

/// Used by stack ensure to attach actionable messaging without a second full probe if needed.
pub fn docker_status_snapshot() -> DockerStatus {
    build_status()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_url_is_https() {
        let (url, hint) = install_meta();
        assert!(url.starts_with("https://"));
        assert!(!hint.is_empty());
    }

    #[test]
    fn status_struct_serializes() {
        let st = DockerStatus {
            cli_ok: false,
            daemon_ok: false,
            compose_ok: false,
            overall_ok: false,
            detail: "test".into(),
            install_url: "https://example.com".into(),
            install_hint: "hint".into(),
            platform: "linux".into(),
        };
        let v = serde_json::to_value(&st).unwrap();
        assert_eq!(v["overall_ok"], false);
    }

    #[test]
    fn probe_runs_without_panic() {
        // May be true or false depending on host — must not panic.
        let _ = Duration::from_secs(1);
        let st = get_docker_status();
        assert!(!st.install_url.is_empty());
        assert!(!st.platform.is_empty());
    }
}
