//! Cloud login session for official-key relay mode (2026-08-15 wave, W3).
//!
//! Login happens **Rust-side via reqwest** (not WebView fetch) so there is no
//! CORS dependency on the cloud API. Sequence: cloud session JWT → mint a
//! desktop token (`cos_dt_*`, relay-only) → fetch server-driven relay
//! coordinates → persist `cloud_session.json` (0600, app data dir, same
//! pattern as `local_session.rs`) → regenerate `client.env` → restart the
//! local product if it is already running so api/worker pick up relay creds.
//!
//! Cloud base: `CONTEXT_OS_CLOUD_BASE` override, default `https://app.contextlm.top`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Manager;

use super::api::IpcApiError;

const DEFAULT_CLOUD_BASE: &str = "https://app.contextlm.top";
const SESSION_FILENAME: &str = "cloud_session.json";
/// Bundle identifier from `tauri.conf.json` (tauri `app_data_dir` =
/// `dirs::data_dir()` + identifier). Keep in sync if the identifier changes.
const BUNDLE_IDENTIFIER: &str = "com.contextos.desktop";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudUser {
    pub id: String,
    pub email: String,
    pub full_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudRelayConfig {
    pub base_url: String,
    pub chat_model: String,
    pub embedding_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoredCloudSession {
    pub cloud_base: String,
    pub user: CloudUser,
    pub session_token: String,
    /// Token row id, needed for best-effort cloud-side revoke on logout.
    pub desktop_token_id: String,
    pub desktop_token: String,
    pub relay: CloudRelayConfig,
}

impl StoredCloudSession {
    /// Well-formed = every field the client.env injection needs is present.
    fn well_formed(self) -> Option<Self> {
        if self.session_token.is_empty()
            || self.desktop_token.is_empty()
            || self.relay.base_url.is_empty()
            || self.relay.chat_model.is_empty()
            || self.relay.embedding_model.is_empty()
        {
            return None;
        }
        Some(self)
    }
}

/// Redacted view for the frontend gate/drawer — never carries tokens.
#[derive(Debug, Clone, Serialize)]
pub struct CloudSessionView {
    pub logged_in: bool,
    pub cloud_base: String,
    pub user: Option<CloudUser>,
    pub relay: Option<CloudRelayConfig>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CloudLoginResult {
    pub user: CloudUser,
    pub relay: CloudRelayConfig,
    pub env_updated: bool,
    pub product_restarted: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CloudLogoutResult {
    pub logged_out: bool,
    pub env_updated: bool,
    pub product_restarted: bool,
    pub message: String,
}

fn cloud_base() -> String {
    std::env::var("CONTEXT_OS_CLOUD_BASE")
        .ok()
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_CLOUD_BASE.into())
}

fn session_path(app: &tauri::AppHandle) -> Result<PathBuf, IpcApiError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| IpcApiError::internal(format!("app_data_dir: {e}")))?;
    Ok(dir.join(SESSION_FILENAME))
}

/// App data dir without an AppHandle — mirrors tauri PathResolver
/// (`dirs::data_dir()` + bundle identifier). Used by `native_stack` when
/// writing client.env outside a command context.
fn app_data_dir_standalone() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join(BUNDLE_IDENTIFIER))
}

fn load_session(app: &tauri::AppHandle) -> Option<StoredCloudSession> {
    let path = session_path(app).ok()?;
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<StoredCloudSession>(&raw)
        .ok()
        .and_then(StoredCloudSession::well_formed)
}

/// Session loader for `native_stack` (client.env injection path): no AppHandle,
/// no network — a well-formed on-disk file is enough.
pub(crate) fn load_session_standalone() -> Option<StoredCloudSession> {
    let path = app_data_dir_standalone()?.join(SESSION_FILENAME);
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<StoredCloudSession>(&raw)
        .ok()
        .and_then(StoredCloudSession::well_formed)
}

fn save_session(app: &tauri::AppHandle, session: &StoredCloudSession) -> Result<(), IpcApiError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| IpcApiError::internal(format!("app_data_dir: {e}")))?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| IpcApiError::internal(format!("create app data: {e}")))?;
    let path = dir.join(SESSION_FILENAME);
    let raw = serde_json::to_string_pretty(session)
        .map_err(|e| IpcApiError::internal(format!("serialize cloud session: {e}")))?;
    std::fs::write(&path, raw)
        .map_err(|e| IpcApiError::internal(format!("write cloud session: {e}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn device_hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

fn server_message(value: &serde_json::Value, fallback: &str) -> String {
    value
        .get("message")
        .or_else(|| value.get("error").and_then(|e| e.get("message")))
        .and_then(|m| m.as_str())
        .unwrap_or(fallback)
        .to_string()
}

async fn cloud_http_json(
    method: &str,
    url: &str,
    body: Option<serde_json::Value>,
    token: Option<&str>,
) -> Result<(u16, serde_json::Value), IpcApiError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| IpcApiError::internal(format!("http client: {e}")))?;

    let mut req = match method {
        "GET" => client.get(url),
        "POST" => client.post(url),
        _ => {
            return Err(IpcApiError::bad_request(
                "method_not_allowed",
                format!("unsupported {method}"),
            ));
        }
    };
    if let Some(t) = token {
        req = req.bearer_auth(t);
    }
    if let Some(b) = body {
        req = req.json(&b);
    }
    let resp = req.send().await.map_err(|e| {
        if e.is_connect() || e.is_timeout() {
            IpcApiError::service_unavailable(format!(
                "云端不可达（{e}）。官方模型（走余额）需要联网登录；请检查网络后重试。"
            ))
        } else {
            IpcApiError::internal(format!("cloud request failed: {e}"))
        }
    })?;
    let status = resp.status().as_u16();
    let text = resp
        .text()
        .await
        .map_err(|e| IpcApiError::internal(format!("read body: {e}")))?;
    let value = if text.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(&text).unwrap_or_else(|_| serde_json::json!({ "raw": text }))
    };
    Ok((status, value))
}

fn extract_auth_payload(value: &serde_json::Value) -> Option<(String, CloudUser)> {
    let data = value.get("data")?;
    let token = data.get("token")?.as_str()?.to_string();
    let user = data.get("user")?;
    Some((
        token,
        CloudUser {
            id: user.get("id")?.as_str()?.to_string(),
            email: user.get("email")?.as_str()?.to_string(),
            full_name: user
                .get("full_name")
                .and_then(|n| n.as_str())
                .unwrap_or_default()
                .to_string(),
        },
    ))
}

/// Regenerate client.env + restart the local product when it is already
/// running, so api/worker observe the new relay env (or its absence after
/// logout). Best-effort: login/logout itself already succeeded at this point.
async fn apply_env_and_restart() -> (bool, bool, Option<String>) {
    let env_updated = match super::native_stack::refresh_client_env() {
        Ok(_) => true,
        Err(e) => {
            tracing::warn!(error = %e, "cloud session: client.env refresh failed (stack not initialized yet?)");
            false
        }
    };
    let status = super::local_product::get_local_product_status();
    if !(status.api_ok || status.worker_ok) {
        return (env_updated, false, None);
    }
    match super::local_product::restart_local_product().await {
        Ok(result) if result.ok => (env_updated, true, None),
        Ok(result) => (
            env_updated,
            false,
            Some(format!("本机产品重启未完成：{}", result.message)),
        ),
        Err(e) => (env_updated, false, Some(format!("本机产品重启失败：{}", e.message))),
    }
}

#[tauri::command]
pub async fn cloud_login(
    app: tauri::AppHandle,
    email: String,
    password: String,
) -> Result<CloudLoginResult, IpcApiError> {
    let email = email.trim().to_string();
    if email.is_empty() || password.is_empty() {
        return Err(IpcApiError::bad_request(
            "cloud_credentials_empty",
            "请输入云账户邮箱与密码",
        ));
    }
    let base = cloud_base();

    // 1) Cloud session JWT.
    let login_url = format!("{base}/api/auth/login");
    let (status, value) = cloud_http_json(
        "POST",
        &login_url,
        Some(serde_json::json!({ "email": email, "password": password })),
        None,
    )
    .await?;
    if status == 401 || status == 403 {
        return Err(IpcApiError::new(
            status,
            "cloud_auth_failed",
            "邮箱或密码错误",
        ));
    }
    if status >= 300 {
        return Err(IpcApiError::new(
            status,
            "cloud_login_failed",
            server_message(&value, "云登录失败"),
        ));
    }
    let (session_token, user) = extract_auth_payload(&value)
        .ok_or_else(|| IpcApiError::internal("cloud login response missing token/user"))?;

    // 2) Mint the desktop token (plaintext cos_dt_* returned exactly once).
    //    On failure after this point an orphan token may remain cloud-side;
    //    the user can revoke it from cloud settings (v1 accepted).
    let token_name = format!("desktop:{}", device_hostname());
    let tokens_url = format!("{base}/api/v1/desktop/tokens");
    let (status, value) = cloud_http_json(
        "POST",
        &tokens_url,
        Some(serde_json::json!({ "name": token_name })),
        Some(&session_token),
    )
    .await?;
    if status >= 300 {
        return Err(IpcApiError::new(
            status,
            "desktop_token_mint_failed",
            server_message(&value, "桌面令牌签发失败"),
        ));
    }
    let data = value.get("data").cloned().unwrap_or_default();
    let desktop_token = data
        .get("token")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let desktop_token_id = data
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    if desktop_token.is_empty() || desktop_token_id.is_empty() {
        return Err(IpcApiError::internal(
            "desktop token response missing token/id",
        ));
    }

    // 3) Server-driven relay coordinates — the shell never hardcodes models.
    let relay_url = format!("{base}/api/v1/desktop/relay-config");
    let (status, value) = cloud_http_json("GET", &relay_url, None, Some(&session_token)).await?;
    if status >= 300 {
        return Err(IpcApiError::new(
            status,
            "relay_config_failed",
            server_message(&value, "获取官方模型配置失败"),
        ));
    }
    let data = value.get("data").cloned().unwrap_or_default();
    let relay = CloudRelayConfig {
        base_url: data
            .get("relay_base_url")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        chat_model: data
            .get("chat_model")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        embedding_model: data
            .get("embedding_model")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
    };
    if relay.base_url.is_empty() || relay.chat_model.is_empty() || relay.embedding_model.is_empty()
    {
        return Err(IpcApiError::internal("relay-config response incomplete"));
    }

    // 4) Persist the session (0600).
    let session = StoredCloudSession {
        cloud_base: base,
        user,
        session_token,
        desktop_token_id,
        desktop_token,
        relay,
    };
    save_session(&app, &session)?;

    // 5) client.env + local product restart.
    let (env_updated, product_restarted, note) = apply_env_and_restart().await;

    Ok(CloudLoginResult {
        user: session.user,
        relay: session.relay,
        env_updated,
        product_restarted,
        message: match note {
            Some(note) => format!("云登录成功。{note}"),
            None => "云登录成功，官方模型（走余额）已就绪".into(),
        },
    })
}

#[tauri::command]
pub async fn cloud_logout(app: tauri::AppHandle) -> Result<CloudLogoutResult, IpcApiError> {
    // Best-effort cloud-side revoke (network down must not block local logout).
    if let Some(session) = load_session(&app) {
        let url = format!(
            "{}/api/v1/desktop/tokens/{}/revoke",
            session.cloud_base, session.desktop_token_id
        );
        let _ = cloud_http_json(
            "POST",
            &url,
            Some(serde_json::json!({})),
            Some(&session.session_token),
        )
        .await;
    }

    let path = session_path(&app)?;
    if path.is_file() {
        std::fs::remove_file(&path)
            .map_err(|e| IpcApiError::internal(format!("remove cloud session: {e}")))?;
    }

    let (env_updated, product_restarted, _) = apply_env_and_restart().await;
    Ok(CloudLogoutResult {
        logged_out: true,
        env_updated,
        product_restarted,
        message: "已退出云登录，本机将不再使用官方模型（走余额）".into(),
    })
}

#[tauri::command]
pub async fn get_cloud_session(app: tauri::AppHandle) -> Result<CloudSessionView, IpcApiError> {
    match load_session(&app) {
        Some(session) => Ok(CloudSessionView {
            logged_in: true,
            cloud_base: session.cloud_base,
            user: Some(session.user),
            relay: Some(session.relay),
            message: "Cloud session active".into(),
        }),
        None => Ok(CloudSessionView {
            logged_in: false,
            cloud_base: cloud_base(),
            user: None,
            relay: None,
            message: "No cloud session".into(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_base_trims_and_defaults() {
        // SAFETY: single-threaded test access pattern in this module.
        unsafe { std::env::remove_var("CONTEXT_OS_CLOUD_BASE") };
        assert_eq!(cloud_base(), DEFAULT_CLOUD_BASE);
        unsafe { std::env::set_var("CONTEXT_OS_CLOUD_BASE", "https://staging.example.com/") };
        assert_eq!(cloud_base(), "https://staging.example.com");
        unsafe { std::env::remove_var("CONTEXT_OS_CLOUD_BASE") };
    }

    #[test]
    fn well_formed_requires_relay_triplet() {
        let session = StoredCloudSession {
            cloud_base: DEFAULT_CLOUD_BASE.into(),
            user: CloudUser {
                id: "u".into(),
                email: "e".into(),
                full_name: "n".into(),
            },
            session_token: "jwt".into(),
            desktop_token_id: "id".into(),
            desktop_token: "cos_dt_x".into(),
            relay: CloudRelayConfig {
                base_url: "https://app.contextlm.top/v1/relay".into(),
                chat_model: "deepseek-v4-flash".into(),
                embedding_model: "BAAI/bge-m3".into(),
            },
        };
        assert!(session.clone().well_formed().is_some());
        let mut broken = session;
        broken.relay.chat_model = String::new();
        assert!(broken.well_formed().is_none());
    }

    #[test]
    fn extract_auth_payload_shape() {
        let value = serde_json::json!({
            "data": {
                "token": "jwt-1",
                "user": {"id": "u1", "email": "a@b.c", "full_name": "A"}
            }
        });
        let (token, user) = extract_auth_payload(&value).expect("payload");
        assert_eq!(token, "jwt-1");
        assert_eq!(user.email, "a@b.c");
        assert!(extract_auth_payload(&serde_json::json!({"data": {}})).is_none());
    }
}
