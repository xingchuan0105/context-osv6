//! Local B2C personal account for the desktop product API (no cloud login).
//!
//! On first licensed run, register `local@context-os.client` against the local
//! avrag-api (or log in if already present). Persist JWT under app data dir.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Manager;

use super::api::IpcApiError;
use super::local_product::product_api_base_url;

const LOCAL_EMAIL: &str = "local@context-os.client";
const LOCAL_FULL_NAME: &str = "Local User";
/// Keep in sync with `app_core::PUBLISHED_*` / frontend legal versions.
const TERMS_VERSION: &str = "2026-06-13";
const PRIVACY_VERSION: &str = "2026-06-13";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalAuthUser {
    pub id: String,
    pub email: String,
    pub full_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredCredentials {
    email: String,
    password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredSession {
    token: String,
    user: LocalAuthUser,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalSessionStatus {
    pub ready: bool,
    pub email: String,
    pub token: Option<String>,
    pub user: Option<LocalAuthUser>,
    pub message: String,
    pub api_base_url: String,
}

fn creds_path(app: &tauri::AppHandle) -> Result<PathBuf, IpcApiError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| IpcApiError::internal(format!("app_data_dir: {e}")))?;
    Ok(dir.join("local_user.json"))
}

fn session_path(app: &tauri::AppHandle) -> Result<PathBuf, IpcApiError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| IpcApiError::internal(format!("app_data_dir: {e}")))?;
    Ok(dir.join("local_session.json"))
}

fn ensure_app_data(app: &tauri::AppHandle) -> Result<PathBuf, IpcApiError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| IpcApiError::internal(format!("app_data_dir: {e}")))?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| IpcApiError::internal(format!("create app data: {e}")))?;
    Ok(dir)
}

fn random_password() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    let mut hasher = DefaultHasher::new();
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
        .hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    // 32 hex chars (>= 8) — stable enough for local single-user install.
    format!("{:x}{:x}", hasher.finish(), hasher.finish().wrapping_mul(0x9e37))
}

fn load_or_create_credentials(app: &tauri::AppHandle) -> Result<StoredCredentials, IpcApiError> {
    ensure_app_data(app)?;
    let path = creds_path(app)?;
    if path.is_file() {
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| IpcApiError::internal(format!("read local_user: {e}")))?;
        let creds: StoredCredentials = serde_json::from_str(&raw)
            .map_err(|e| IpcApiError::internal(format!("parse local_user: {e}")))?;
        if creds.password.len() >= 8 && !creds.email.is_empty() {
            return Ok(creds);
        }
    }
    let creds = StoredCredentials {
        email: LOCAL_EMAIL.into(),
        password: random_password(),
    };
    let raw = serde_json::to_string_pretty(&creds)
        .map_err(|e| IpcApiError::internal(format!("serialize local_user: {e}")))?;
    std::fs::write(&path, raw)
        .map_err(|e| IpcApiError::internal(format!("write local_user: {e}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(creds)
}

fn load_session(app: &tauri::AppHandle) -> Option<StoredSession> {
    let path = session_path(app).ok()?;
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

/// JWT of the local B2C session (for shell-orchestrated API calls like reindex).
pub(crate) fn local_session_token(app: &tauri::AppHandle) -> Option<String> {
    load_session(app).map(|session| session.token)
}

fn save_session(app: &tauri::AppHandle, session: &StoredSession) -> Result<(), IpcApiError> {
    ensure_app_data(app)?;
    let path = session_path(app)?;
    let raw = serde_json::to_string_pretty(session)
        .map_err(|e| IpcApiError::internal(format!("serialize session: {e}")))?;
    std::fs::write(&path, raw).map_err(|e| IpcApiError::internal(format!("write session: {e}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

async fn http_json(
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
        if e.is_connect() {
            IpcApiError::service_unavailable(format!(
                "Local product API unreachable ({e}). Start: bash scripts/desktop-local-product.sh ensure"
            ))
        } else {
            IpcApiError::internal(format!("request failed: {e}"))
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

fn extract_auth_payload(value: &serde_json::Value) -> Option<(String, LocalAuthUser)> {
    let data = value.get("data")?;
    let token = data.get("token")?.as_str()?.to_string();
    let user = data.get("user")?;
    Some((
        token,
        LocalAuthUser {
            id: user.get("id")?.as_str()?.to_string(),
            email: user
                .get("email")
                .and_then(|e| e.as_str())
                .unwrap_or(LOCAL_EMAIL)
                .to_string(),
            full_name: user
                .get("full_name")
                .and_then(|e| e.as_str())
                .unwrap_or(LOCAL_FULL_NAME)
                .to_string(),
        },
    ))
}

async fn me_ok(base: &str, token: &str) -> bool {
    let url = format!("{base}/api/auth/me");
    match http_json("GET", &url, None, Some(token)).await {
        Ok((status, value)) => {
            status < 300
                && value
                    .get("success")
                    .and_then(|s| s.as_bool())
                    .unwrap_or(false)
        }
        Err(_) => false,
    }
}

async fn login_or_register(
    base: &str,
    creds: &StoredCredentials,
) -> Result<StoredSession, IpcApiError> {
    let login_url = format!("{base}/api/auth/login");
    let (status, value) = http_json(
        "POST",
        &login_url,
        Some(serde_json::json!({
            "email": creds.email,
            "password": creds.password,
        })),
        None,
    )
    .await?;

    if status < 300 {
        if let Some((token, user)) = extract_auth_payload(&value) {
            return Ok(StoredSession { token, user });
        }
    }

    // Not registered or wrong password → try register (idempotent path for first boot).
    let register_url = format!("{base}/api/auth/register");
    let (r_status, r_value) = http_json(
        "POST",
        &register_url,
        Some(serde_json::json!({
            "email": creds.email,
            "password": creds.password,
            "full_name": LOCAL_FULL_NAME,
            "terms_version": TERMS_VERSION,
            "privacy_version": PRIVACY_VERSION,
            "local": true,
        })),
        None,
    )
    .await?;

    if r_status < 300 {
        if let Some((token, user)) = extract_auth_payload(&r_value) {
            return Ok(StoredSession { token, user });
        }
    }

    // Retry login once more (race: another process registered).
    let (status2, value2) = http_json(
        "POST",
        &login_url,
        Some(serde_json::json!({
            "email": creds.email,
            "password": creds.password,
        })),
        None,
    )
    .await?;
    if status2 < 300 {
        if let Some((token, user)) = extract_auth_payload(&value2) {
            return Ok(StoredSession { token, user });
        }
    }

    let msg = r_value
        .get("message")
        .or_else(|| value.get("message"))
        .and_then(|m| m.as_str())
        .unwrap_or("local register/login failed");
    Err(IpcApiError::new(
        if r_status >= 400 { r_status } else { status },
        "local_session_failed",
        msg.to_string(),
    ))
}

#[tauri::command]
pub async fn get_local_session(app: tauri::AppHandle) -> Result<LocalSessionStatus, IpcApiError> {
    let base = product_api_base_url();
    if let Some(session) = load_session(&app) {
        if me_ok(&base, &session.token).await {
            return Ok(LocalSessionStatus {
                ready: true,
                email: session.user.email.clone(),
                token: Some(session.token),
                user: Some(session.user),
                message: "Local session valid".into(),
                api_base_url: base,
            });
        }
    }
    Ok(LocalSessionStatus {
        ready: false,
        email: LOCAL_EMAIL.into(),
        token: None,
        user: None,
        message: "No valid local session".into(),
        api_base_url: base,
    })
}

/// Bring up data plane + product if the local API is not healthy.
async fn ensure_local_environment() -> Result<(), IpcApiError> {
    let base = product_api_base_url();
    let health_url = format!("{base}/health");
    if http_json("GET", &health_url, None, None).await.is_ok() {
        return Ok(());
    }

    // Stack first (PG + Redis + client.env), then product (api + worker).
    let stack = super::local_stack::ensure_local_stack().await?;
    if !stack.ok {
        return Err(IpcApiError::service_unavailable(format!(
            "本机数据面未就绪：{}",
            stack.message
        )));
    }

    let product = super::local_product::ensure_local_product().await?;
    if !product.ok {
        return Err(IpcApiError::service_unavailable(format!(
            "本机产品进程未就绪：{}",
            product.message
        )));
    }

    // Re-check health after ensure.
    if http_json("GET", &health_url, None, None).await.is_err() {
        return Err(IpcApiError::service_unavailable(format!(
            "本机产品 API 仍不可达（{base}/health）。请查看设置中的产品日志。"
        )));
    }
    Ok(())
}

/// Ensure a personal B2C user exists on the local product API and return a JWT.
/// On cold start, automatically brings up local stack + product when the API is down.
#[tauri::command]
pub async fn ensure_local_session(app: tauri::AppHandle) -> Result<LocalSessionStatus, IpcApiError> {
    let base = product_api_base_url();

    if let Some(session) = load_session(&app) {
        if me_ok(&base, &session.token).await {
            return Ok(LocalSessionStatus {
                ready: true,
                email: session.user.email.clone(),
                token: Some(session.token),
                user: Some(session.user),
                message: "Local session already active".into(),
                api_base_url: base,
            });
        }
    }

    ensure_local_environment().await?;

    let base = product_api_base_url();
    let creds = load_or_create_credentials(&app)?;
    let session = login_or_register(&base, &creds).await?;
    save_session(&app, &session)?;

    Ok(LocalSessionStatus {
        ready: true,
        email: session.user.email.clone(),
        token: Some(session.token),
        user: Some(session.user),
        message: "Local B2C session ready (personal account, no cloud login)".into(),
        api_base_url: base,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_password_long_enough() {
        assert!(random_password().len() >= 16);
    }

    #[test]
    fn local_email_is_personal_not_org() {
        assert!(LOCAL_EMAIL.contains("context-os"));
        assert!(!LOCAL_EMAIL.contains("org"));
    }
}
