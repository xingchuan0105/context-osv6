//! Persist / discover user tokens for CLI and stdio MCP.
//!
//! Resolution order for user token (after explicit env):
//! 1. `CONTEXT_OS_USER_TOKEN_FILE` path
//! 2. Default config file (`~/.config/context-os/user.token`)
//! 3. Desktop client `local_session.json` (optional auto-load)

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct DesktopSessionFile {
    token: String,
    #[serde(default)]
    user: Option<DesktopSessionUser>,
}

#[derive(Debug, Deserialize)]
struct DesktopSessionUser {
    #[serde(default)]
    email: Option<String>,
}

/// `~/.config/context-os/user.token` (or `$XDG_CONFIG_HOME` / Windows APPDATA).
pub fn default_token_file() -> PathBuf {
    if let Ok(p) = std::env::var("CONTEXT_OS_USER_TOKEN_FILE") {
        let p = p.trim();
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    config_dir().join("user.token")
}

pub fn config_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        let p = PathBuf::from(xdg);
        if !p.as_os_str().is_empty() {
            return p.join("context-os");
        }
    }
    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("context-os");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".config").join("context-os");
    }
    PathBuf::from(".context-os")
}

/// Candidate paths for Tauri / install-layout `local_session.json`.
pub fn desktop_session_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(p) = std::env::var("CONTEXT_OS_DESKTOP_SESSION") {
        let p = p.trim();
        if !p.is_empty() {
            out.push(PathBuf::from(p));
        }
    }
    if let Ok(state) = std::env::var("CONTEXT_OS_STATE_HOME") {
        let state = PathBuf::from(state);
        out.push(state.join("local_session.json"));
    }

    #[cfg(windows)]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let local = PathBuf::from(local);
            out.push(
                local
                    .join("com.contextos.desktop")
                    .join("local_session.json"),
            );
            out.push(
                local
                    .join("Context-OS Client")
                    .join("local_session.json"),
            );
        }
    }

    #[cfg(not(windows))]
    {
        if let Ok(home) = std::env::var("HOME") {
            let home = PathBuf::from(home);
            // Tauri 2 app_data_dir with identifier com.contextos.desktop
            out.push(
                home.join(".local/share/com.contextos.desktop/local_session.json"),
            );
            out.push(home.join(".local/share/context-os-client/local_session.json"));
            out.push(home.join(".local/share/Context-OS Client/local_session.json"));
            // macOS Application Support
            out.push(
                home.join("Library/Application Support/com.contextos.desktop/local_session.json"),
            );
            out.push(
                home.join("Library/Application Support/Context-OS Client/local_session.json"),
            );
        }
    }

    out
}

pub fn read_token_file(path: &Path) -> Result<Option<String>> {
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("read token file {}", path.display()))?;
    let token = raw
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .unwrap_or("")
        .to_string();
    if token.is_empty() {
        Ok(None)
    } else {
        Ok(Some(token))
    }
}

pub fn write_token_file(path: &Path, token: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        let parent_existed = parent.is_dir();
        fs::create_dir_all(parent)
            .with_context(|| format!("create {}", parent.display()))?;
        // Only tighten mode on our owned config leaf (`…/context-os`), never
        // shared parents (e.g. CONTEXT_OS_USER_TOKEN_FILE under /tmp).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let owned_config = config_dir();
            let is_owned_leaf = parent == owned_config.as_path()
                || parent.file_name().and_then(|n| n.to_str()) == Some("context-os");
            if is_owned_leaf && !parent_existed {
                fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).with_context(
                    || format!("chmod 0700 config dir {}", parent.display()),
                )?;
            } else if is_owned_leaf {
                // Best-effort tighten existing owned dir; ignore failures (e.g. not owner).
                let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
            }
        }
    }
    let body = format!(
        "# Context-OS user / agent token — do not commit\n# path: {}\n{}\n",
        path.display(),
        token.trim()
    );
    // Atomic-ish: write temp with 0600 then rename (never create world-readable JWT).
    let tmp = path.with_extension("token.tmp");
    {
        use std::io::Write;
        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            let mut f = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&tmp)
                .with_context(|| format!("create temp token {}", tmp.display()))?;
            f.write_all(body.as_bytes())
                .with_context(|| format!("write temp token {}", tmp.display()))?;
            f.sync_all().ok();
            // Reinforce mode in case umask interacted oddly on some platforms.
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))
                .with_context(|| format!("chmod 0600 {}", tmp.display()))?;
        }
        #[cfg(not(unix))]
        {
            fs::write(&tmp, body.as_bytes())
                .with_context(|| format!("write temp token {}", tmp.display()))?;
        }
    }
    // Windows fails rename if destination exists; remove first on all platforms.
    if path.exists() {
        fs::remove_file(path)
            .with_context(|| format!("remove existing token {}", path.display()))?;
    }
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e).with_context(|| {
            format!(
                "rename {} → {}",
                tmp.display(),
                path.display()
            )
        });
    }
    Ok(())
}

pub fn load_default_token_file() -> Result<Option<String>> {
    read_token_file(&default_token_file())
}

#[derive(Debug, Clone)]
pub struct DesktopSessionToken {
    pub token: String,
    pub path: PathBuf,
    pub email: Option<String>,
}

pub fn load_desktop_session_token() -> Result<Option<DesktopSessionToken>> {
    for path in desktop_session_candidates() {
        if !path.is_file() {
            continue;
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("read desktop session {}", path.display()))?;
        let parsed: DesktopSessionFile = serde_json::from_str(&raw)
            .with_context(|| format!("parse desktop session {}", path.display()))?;
        let token = parsed.token.trim().to_string();
        if token.is_empty() {
            continue;
        }
        return Ok(Some(DesktopSessionToken {
            token,
            path,
            email: parsed.user.and_then(|u| u.email),
        }));
    }
    Ok(None)
}

/// Resolve user token from env file / default file / desktop session.
/// Does not read CONTEXT_OS_USER_TOKEN itself (caller handles env first).
pub fn discover_user_token(load_desktop: bool) -> Result<Option<String>> {
    if let Some(t) = load_default_token_file()? {
        return Ok(Some(t));
    }
    if load_desktop {
        if let Some(s) = load_desktop_session_token()? {
            return Ok(Some(s.token));
        }
    }
    Ok(None)
}

pub fn save_user_token(token: &str) -> Result<PathBuf> {
    let path = default_token_file();
    if token.trim().is_empty() {
        bail!("refusing to write empty token");
    }
    write_token_file(&path, token)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn read_token_skips_comments() {
        let dir = std::env::temp_dir().join(format!("context-os-token-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("user.token");
        let mut f = fs::File::create(&path).unwrap();
        writeln!(f, "# comment").unwrap();
        writeln!(f, "abc.jwt.token").unwrap();
        let t = read_token_file(&path).unwrap().unwrap();
        assert_eq!(t, "abc.jwt.token");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn desktop_candidates_nonempty() {
        assert!(!desktop_session_candidates().is_empty());
    }

    #[test]
    fn write_token_overwrites_existing() {
        let dir = std::env::temp_dir().join(format!(
            "context-os-token-ow-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("user.token");
        write_token_file(&path, "first.jwt").unwrap();
        write_token_file(&path, "second.jwt").unwrap();
        let t = read_token_file(&path).unwrap().unwrap();
        assert_eq!(t, "second.jwt");
        let _ = fs::remove_dir_all(&dir);
    }
}
