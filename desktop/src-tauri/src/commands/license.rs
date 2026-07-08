use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use keygen_rs::config::{self, KeygenConfig};
use keygen_rs::errors::Error as KeygenError;
use keygen_rs::license::SchemeCode;
use machineid_rs::{Encryption, HWIDComponent, IdBuilder};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::ShellExt;
use url::Url;

const LICENSE_FILENAME: &str = "license.json";
const DEVICE_SALT: &str = "avrag-desktop-salt";
const TRIAL_DURATION_SECS: i64 = 7 * 24 * 60 * 60;
const OFFLINE_GRACE_SECS: i64 = 30 * 24 * 60 * 60;
const HEARTBEAT_INTERVAL_SECS: i64 = 24 * 60 * 60;

const KEYGEN_PUBLIC_KEY: Option<&str> = option_env!("KEYGEN_PUBLIC_KEY");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseKind {
    Trial,
    Standard,
    Pro,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseStatusKind {
    Unactivated,
    Trial,
    Active,
    Expired,
    Revoked,
    OfflineGrace,
    UpgradeRequired,
}

pub fn license_allows_chat(kind: LicenseStatusKind) -> bool {
    matches!(
        kind,
        LicenseStatusKind::Active
            | LicenseStatusKind::Trial
            | LicenseStatusKind::OfflineGrace
            | LicenseStatusKind::UpgradeRequired
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseError {
    pub code: String,
    pub message: String,
}

impl LicenseError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl From<String> for LicenseError {
    fn from(message: String) -> Self {
        Self::new("internal_error", message)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateClaims {
    pub device_id: String,
    pub expires_at: Option<i64>,
    pub major_version_included: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseFile {
    pub key: String,
    pub license_id: String,
    pub device_id: String,
    #[serde(default)]
    pub machine_id: Option<String>,
    pub certificate: String,
    pub kind: LicenseKind,
    pub issued_at: i64,
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub last_heartbeat: Option<i64>,
    #[serde(default)]
    pub revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialResult {
    pub expires_at: i64,
    pub days_remaining: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivationResult {
    pub license_id: String,
    pub kind: LicenseKind,
    pub status: LicenseStatusKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatResult {
    pub success: bool,
    pub status: LicenseStatusKind,
    pub next_heartbeat_at: Option<i64>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseStatus {
    pub kind: LicenseStatusKind,
    pub days_remaining: Option<i32>,
    pub offline_grace_days: Option<i32>,
    pub license_kind: Option<LicenseKind>,
    pub expires_at: Option<i64>,
    pub dev_mode: bool,
}

pub fn is_dev_mode() -> bool {
    KEYGEN_PUBLIC_KEY
        .map(str::trim)
        .is_none_or(str::is_empty)
}

pub fn compute_device_id() -> Result<String, String> {
    let mut builder = IdBuilder::new(Encryption::SHA256);
    builder
        .add_component(HWIDComponent::SystemID)
        .add_component(HWIDComponent::CPUCores)
        .add_component(HWIDComponent::DriveSerial);
    builder.build(DEVICE_SALT).map_err(|e| e.to_string())
}

pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub fn app_major_version() -> u32 {
    env!("CARGO_PKG_VERSION")
        .split('.')
        .next()
        .and_then(|part| part.parse().ok())
        .unwrap_or(1)
}

pub fn days_remaining(until: i64, now: i64) -> i32 {
    ((until - now).max(0) / 86_400) as i32
}

pub fn offline_grace_days(last_heartbeat: i64, now: i64) -> i32 {
    let remaining = OFFLINE_GRACE_SECS - (now - last_heartbeat);
    (remaining.max(0) / 86_400) as i32
}

pub fn parse_activate_key(raw_url: &str) -> Option<String> {
    let parsed = Url::parse(raw_url).ok()?;
    if parsed.scheme() != "avrag-desktop" {
        return None;
    }

    let is_activate = parsed.host_str() == Some("activate")
        || parsed.path().trim_end_matches('/') == "/activate";
    if !is_activate {
        return None;
    }

    parsed
        .query_pairs()
        .find(|(key, _)| key == "key")
        .map(|(_, value)| value.into_owned())
}

pub fn handle_deep_link_url(app: &AppHandle, raw_url: &str) {
    if let Some(key) = parse_activate_key(raw_url) {
        let _ = app.emit(
            "deep-link-activate",
            serde_json::json!({
                "key": key,
            }),
        );
    }
}

fn license_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(LICENSE_FILENAME)
}

pub fn load_license_file(app_data_dir: &Path) -> Result<Option<LicenseFile>, String> {
    let path = license_path(app_data_dir);
    if !path.exists() {
        return Ok(None);
    }

    let contents = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read license file: {e}"))?;
    serde_json::from_str(&contents).map_err(|e| format!("Invalid license file: {e}"))
}

pub fn save_license_file(app_data_dir: &Path, file: &LicenseFile) -> Result<(), String> {
    std::fs::create_dir_all(app_data_dir)
        .map_err(|e| format!("Failed to create app data dir: {e}"))?;
    let json = serde_json::to_string_pretty(file)
        .map_err(|e| format!("Failed to serialize license file: {e}"))?;
    std::fs::write(license_path(app_data_dir), json)
        .map_err(|e| format!("Failed to write license file: {e}"))
}

fn decode_certificate_payload(certificate: &str) -> Result<Vec<u8>, LicenseError> {
    let (payload_b64, _) = certificate
        .rsplit_once('.')
        .ok_or_else(|| LicenseError::new("malformed_certificate", "Certificate is malformed"))?;
    URL_SAFE_NO_PAD
        .decode(payload_b64)
        .or_else(|_| URL_SAFE_NO_PAD.decode(payload_b64.replace('-', "+").replace('_', "/")))
        .map_err(|e| LicenseError::new("malformed_certificate", e.to_string()))
}

pub fn verify_certificate(
    certificate: &str,
    device_id: &str,
    dev_mode: bool,
) -> Result<CertificateClaims, LicenseError> {
    if dev_mode {
        let payload = decode_certificate_payload(certificate)?;
        let claims: CertificateClaims = serde_json::from_slice(&payload).map_err(|e| {
            LicenseError::new("malformed_certificate", format!("Invalid dev certificate: {e}"))
        })?;
        if claims.device_id != device_id {
            return Err(LicenseError::new(
                "device_mismatch",
                "Certificate device_id does not match this machine",
            ));
        }
        return Ok(claims);
    }

    let public_key = KEYGEN_PUBLIC_KEY
        .ok_or_else(|| LicenseError::new("public_key_missing", "KEYGEN_PUBLIC_KEY is not set"))?
        .trim();
    if public_key.is_empty() {
        return Err(LicenseError::new(
            "public_key_missing",
            "KEYGEN_PUBLIC_KEY is empty",
        ));
    }

    let (payload_b64, sig_b64) = certificate.rsplit_once('.').ok_or_else(|| {
        LicenseError::new("malformed_certificate", "Certificate is malformed")
    })?;
    let payload = decode_certificate_payload(certificate)?;
    let sig_bytes = URL_SAFE_NO_PAD
        .decode(sig_b64)
        .map_err(|e| LicenseError::new("malformed_certificate", e.to_string()))?;
    let signature = Signature::from_slice(&sig_bytes).map_err(|e| {
        LicenseError::new("malformed_certificate", format!("Invalid signature: {e}"))
    })?;

    let key_bytes = URL_SAFE_NO_PAD
        .decode(public_key)
        .map_err(|e| LicenseError::new("public_key_invalid", e.to_string()))?;
    let verifying_key = VerifyingKey::from_bytes(
        key_bytes
            .as_slice()
            .try_into()
            .map_err(|_| LicenseError::new("public_key_invalid", "Invalid public key length"))?,
    )
    .map_err(|e| LicenseError::new("public_key_invalid", e.to_string()))?;

    verifying_key
        .verify(payload_b64.as_bytes(), &signature)
        .map_err(|_| LicenseError::new("signature_invalid", "Certificate signature is invalid"))?;

    let claims: CertificateClaims = serde_json::from_slice(&payload).map_err(|e| {
        LicenseError::new("malformed_certificate", format!("Invalid certificate payload: {e}"))
    })?;
    if claims.device_id != device_id {
        return Err(LicenseError::new(
            "device_mismatch",
            "Certificate device_id does not match this machine",
        ));
    }

    Ok(claims)
}

pub fn resolve_license_status(
    file: Option<&LicenseFile>,
    device_id: &str,
    now: i64,
    dev_mode: bool,
) -> LicenseStatus {
    let Some(file) = file else {
        return LicenseStatus {
            kind: LicenseStatusKind::Unactivated,
            days_remaining: None,
            offline_grace_days: None,
            license_kind: None,
            expires_at: None,
            dev_mode,
        };
    };

    if file.revoked {
        return status_from_kind(
            LicenseStatusKind::Revoked,
            file,
            now,
            dev_mode,
            None,
            None,
        );
    }

    if file.device_id != device_id {
        return status_from_kind(
            LicenseStatusKind::Unactivated,
            file,
            now,
            dev_mode,
            None,
            None,
        );
    }

    let claims = match verify_certificate(&file.certificate, device_id, dev_mode) {
        Ok(claims) => claims,
        Err(_) => {
            return status_from_kind(
                LicenseStatusKind::Unactivated,
                file,
                now,
                dev_mode,
                None,
                None,
            );
        }
    };

    if file.kind == LicenseKind::Trial {
        let expires_at = file.expires_at.or(claims.expires_at);
        if let Some(expires_at) = expires_at {
            if now >= expires_at {
                return status_from_kind(
                    LicenseStatusKind::Unactivated,
                    file,
                    now,
                    dev_mode,
                    Some(expires_at),
                    None,
                );
            }
            return status_from_kind(
                LicenseStatusKind::Trial,
                file,
                now,
                dev_mode,
                Some(expires_at),
                None,
            );
        }
    }

    if let Some(expires_at) = claims.expires_at {
        if now < expires_at {
            return status_from_kind(
                LicenseStatusKind::Active,
                file,
                now,
                dev_mode,
                Some(expires_at),
                None,
            );
        }

        if let Some(last_heartbeat) = file.last_heartbeat {
            let grace_remaining = OFFLINE_GRACE_SECS - (now - last_heartbeat);
            if grace_remaining > 0 {
                return status_from_kind(
                    LicenseStatusKind::OfflineGrace,
                    file,
                    now,
                    dev_mode,
                    Some(expires_at),
                    Some(offline_grace_days(last_heartbeat, now)),
                );
            }
        }

        return status_from_kind(
            LicenseStatusKind::Expired,
            file,
            now,
            dev_mode,
            Some(expires_at),
            None,
        );
    }

    status_from_kind(
        LicenseStatusKind::Active,
        file,
        now,
        dev_mode,
        file.expires_at,
        None,
    )
}

fn status_from_kind(
    kind: LicenseStatusKind,
    file: &LicenseFile,
    now: i64,
    dev_mode: bool,
    expires_at: Option<i64>,
    offline_grace_days: Option<i32>,
) -> LicenseStatus {
    let expires_at = expires_at.or(file.expires_at);
    LicenseStatus {
        kind,
        days_remaining: expires_at.map(|exp| days_remaining(exp, now)),
        offline_grace_days,
        license_kind: Some(file.kind),
        expires_at,
        dev_mode,
    }
}

fn build_dev_certificate(
    device_id: &str,
    expires_at: Option<i64>,
    major_version_included: u32,
) -> String {
    let claims = CertificateClaims {
        device_id: device_id.to_string(),
        expires_at,
        major_version_included: Some(major_version_included),
    };
    let payload = serde_json::to_vec(&claims).expect("certificate claims must serialize");
    let payload_b64 = URL_SAFE_NO_PAD.encode(payload);
    format!("{payload_b64}.dev")
}

fn infer_license_kind(metadata: &std::collections::HashMap<String, serde_json::Value>) -> LicenseKind {
    metadata
        .get("tier")
        .and_then(|value| value.as_str())
        .map(|tier| match tier.to_ascii_lowercase().as_str() {
            "pro" => LicenseKind::Pro,
            "standard" => LicenseKind::Standard,
            _ => LicenseKind::Standard,
        })
        .unwrap_or(LicenseKind::Standard)
}

fn build_keygen_config(license_key: &str) -> Result<KeygenConfig, LicenseError> {
    let account = std::env::var("KEYGEN_ACCOUNT_ID").map_err(|_| {
        LicenseError::new(
            "keygen_config_missing",
            "KEYGEN_ACCOUNT_ID environment variable is required",
        )
    })?;
    let product = std::env::var("KEYGEN_PRODUCT_ID").map_err(|_| {
        LicenseError::new(
            "keygen_config_missing",
            "KEYGEN_PRODUCT_ID environment variable is required",
        )
    })?;
    let public_key = KEYGEN_PUBLIC_KEY
        .ok_or_else(|| {
            LicenseError::new(
                "public_key_missing",
                "KEYGEN_PUBLIC_KEY must be set at compile time",
            )
        })?
        .trim()
        .to_string();

    let mut config = KeygenConfig::license_key(account, product, license_key.to_string(), public_key);
    config.api_url = std::env::var("KEYGEN_API_URL")
        .unwrap_or_else(|_| "https://license.avrag.com/v1".to_string());
    Ok(config)
}

async fn keygen_validate_and_activate(
    license_key: &str,
    device_id: &str,
) -> Result<(keygen_rs::license::License, Option<String>), LicenseError> {
    let config = build_keygen_config(license_key)?;
    let _ = config::set_config(config.clone());

    match keygen_rs::validate(&[device_id.to_string()], &[]).await {
        Ok(license) => Ok((license, None)),
        Err(KeygenError::LicenseNotActivated { license, .. }) => {
            let machine = license
                .activate(device_id, &[])
                .await
                .map_err(keygen_error_to_license_error)?;
            Ok((license, Some(machine.id)))
        }
        Err(err) => Err(keygen_error_to_license_error(err)),
    }
}

fn keygen_error_to_license_error(err: KeygenError) -> LicenseError {
    let (code, message) = match err {
        KeygenError::LicenseSuspended { code, detail, .. } => (code, detail),
        KeygenError::LicenseExpired { code, detail, .. } => (code, detail),
        KeygenError::LicenseTooManyMachines { code, detail, .. } => (code, detail),
        KeygenError::HeartbeatDead { code, detail, .. } => (code, detail),
        KeygenError::LicenseKeyInvalid { code, detail, .. } => (code, detail),
        other => ("keygen_error".to_string(), other.to_string()),
    };
    LicenseError { code, message }
}

async fn activate_with_keygen(
    license_key: &str,
    device_id: &str,
    app_data_dir: &Path,
) -> Result<ActivationResult, LicenseError> {
    let (license, machine_id) = keygen_validate_and_activate(license_key, device_id).await?;
    let now = now_unix();
    let expires_at = license.expiry.map(|dt| dt.timestamp());
    let kind = infer_license_kind(&license.metadata);

    let certificate = if let Ok(data) = keygen_rs::verify(SchemeCode::Ed25519Sign, &license.key) {
        let payload_b64 = URL_SAFE_NO_PAD.encode(data);
        format!("{payload_b64}.{}", license.key.rsplit('.').next().unwrap_or("keygen"))
    } else {
        build_dev_certificate(device_id, expires_at, app_major_version())
    };

    let file = LicenseFile {
        key: license_key.to_string(),
        license_id: license.id,
        device_id: device_id.to_string(),
        machine_id,
        certificate,
        kind,
        issued_at: now,
        expires_at,
        last_heartbeat: Some(now),
        revoked: false,
    };
    save_license_file(app_data_dir, &file)?;

    Ok(ActivationResult {
        license_id: file.license_id.clone(),
        kind: file.kind,
        status: LicenseStatusKind::Active,
    })
}

async fn mock_activate(
    license_key: &str,
    device_id: &str,
    app_data_dir: &Path,
    kind: LicenseKind,
    expires_at: Option<i64>,
) -> Result<ActivationResult, LicenseError> {
    let now = now_unix();
    let license_id = uuid::Uuid::new_v4().to_string();
    let file = LicenseFile {
        key: license_key.to_string(),
        license_id: license_id.clone(),
        device_id: device_id.to_string(),
        machine_id: Some(format!("dev-machine-{license_id}")),
        certificate: build_dev_certificate(device_id, expires_at, app_major_version()),
        kind,
        issued_at: now,
        expires_at,
        last_heartbeat: Some(now),
        revoked: false,
    };
    save_license_file(app_data_dir, &file)?;

    Ok(ActivationResult {
        license_id,
        kind,
        status: if kind == LicenseKind::Trial {
            LicenseStatusKind::Trial
        } else {
            LicenseStatusKind::Active
        },
    })
}

async fn request_trial_license(device_id: &str) -> Result<String, LicenseError> {
    let base = std::env::var("AVRAG_API_BASE")
        .unwrap_or_else(|_| "https://app.avrag.com".to_string());
    let response = reqwest::Client::new()
        .post(format!("{base}/api/v1/licenses/trial"))
        .json(&serde_json::json!({ "device_id": device_id }))
        .send()
        .await
        .map_err(|e| LicenseError::new("trial_request_failed", e.to_string()))?;

    if !response.status().is_success() {
        return Err(LicenseError::new(
            "trial_request_failed",
            format!("Trial request failed with status {}", response.status()),
        ));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| LicenseError::new("trial_request_failed", e.to_string()))?;
    body.get("license_key")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .ok_or_else(|| {
            LicenseError::new(
                "trial_request_failed",
                "Trial response did not include license_key",
            )
        })
}

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {e}"))
}

#[tauri::command]
pub fn get_device_id() -> Result<String, String> {
    compute_device_id()
}

#[tauri::command]
pub async fn start_trial(app: AppHandle) -> Result<TrialResult, LicenseError> {
    let device_id = compute_device_id().map_err(|e| LicenseError::new("device_id", e))?;
    let app_data_dir = app_data_dir(&app).map_err(|e| LicenseError::new("app_data_dir", e))?;

    if let Some(existing) = load_license_file(&app_data_dir)
        .map_err(|e| LicenseError::new("license_file", e))?
    {
        if existing.device_id == device_id && !existing.revoked {
            let status = resolve_license_status(Some(&existing), &device_id, now_unix(), is_dev_mode());
            if status.kind != LicenseStatusKind::Unactivated {
                return Err(LicenseError::new(
                    "trial_already_used",
                    "This device already has a license or trial",
                ));
            }
        }
    }

    if is_dev_mode() {
        let expires_at = now_unix() + TRIAL_DURATION_SECS;
        mock_activate(
            &format!("DEV-TRIAL-{}", uuid::Uuid::new_v4()),
            &device_id,
            &app_data_dir,
            LicenseKind::Trial,
            Some(expires_at),
        )
        .await?;
        return Ok(TrialResult {
            expires_at,
            days_remaining: 7,
        });
    }

    let license_key = request_trial_license(&device_id).await?;
    activate_with_keygen(&license_key, &device_id, &app_data_dir).await?;
    let expires_at = now_unix() + TRIAL_DURATION_SECS;
    Ok(TrialResult {
        expires_at,
        days_remaining: 7,
    })
}

#[tauri::command]
pub async fn activate_license(
    license_key: String,
    app: AppHandle,
) -> Result<ActivationResult, LicenseError> {
    let device_id = compute_device_id().map_err(|e| LicenseError::new("device_id", e))?;
    let app_data_dir = app_data_dir(&app).map_err(|e| LicenseError::new("app_data_dir", e))?;

    if is_dev_mode() {
        return mock_activate(
            &license_key,
            &device_id,
            &app_data_dir,
            LicenseKind::Standard,
            None,
        )
        .await;
    }

    activate_with_keygen(&license_key, &device_id, &app_data_dir).await
}

#[tauri::command]
pub async fn get_license_status(app: AppHandle) -> Result<LicenseStatus, String> {
    let device_id = compute_device_id()?;
    let app_data_dir = app_data_dir(&app)?;
    let file = load_license_file(&app_data_dir)?;
    Ok(resolve_license_status(file.as_ref(), &device_id, now_unix(), is_dev_mode()))
}

#[tauri::command]
pub async fn heartbeat_license(app: AppHandle) -> Result<HeartbeatResult, String> {
    let device_id = compute_device_id()?;
    let app_data_dir = app_data_dir(&app)?;
    let mut file = load_license_file(&app_data_dir)?.ok_or_else(|| {
        "No license file found".to_string()
    })?;

    let now = now_unix();
    if file.last_heartbeat.is_some_and(|last| now - last < HEARTBEAT_INTERVAL_SECS) {
        let status = resolve_license_status(Some(&file), &device_id, now, is_dev_mode());
        return Ok(HeartbeatResult {
            success: true,
            status: status.kind,
            next_heartbeat_at: file.last_heartbeat.map(|last| last + HEARTBEAT_INTERVAL_SECS),
            message: Some("Heartbeat skipped; interval not reached".to_string()),
        });
    }

    if is_dev_mode() {
        file.last_heartbeat = Some(now);
        save_license_file(&app_data_dir, &file)?;
        let status = resolve_license_status(Some(&file), &device_id, now, true);
        return Ok(HeartbeatResult {
            success: true,
            status: status.kind,
            next_heartbeat_at: Some(now + HEARTBEAT_INTERVAL_SECS),
            message: Some("Dev mode heartbeat recorded".to_string()),
        });
    }

    match keygen_validate_and_activate(&file.key, &device_id).await {
        Ok((license, machine_id)) => {
            file.last_heartbeat = Some(now);
            file.revoked = false;
            if let Some(machine_id) = machine_id {
                file.machine_id = Some(machine_id);
            }
            if let Some(expiry) = license.expiry {
                file.expires_at = Some(expiry.timestamp());
            }
            save_license_file(&app_data_dir, &file)?;
            let status = resolve_license_status(Some(&file), &device_id, now, false);
            Ok(HeartbeatResult {
                success: true,
                status: status.kind,
                next_heartbeat_at: Some(now + HEARTBEAT_INTERVAL_SECS),
                message: None,
            })
        }
        Err(err) if err.code.contains("SUSPEND") || err.code.contains("REVOK") => {
            file.revoked = true;
            save_license_file(&app_data_dir, &file).ok();
            Ok(HeartbeatResult {
                success: false,
                status: LicenseStatusKind::Revoked,
                next_heartbeat_at: None,
                message: Some(err.message),
            })
        }
        Err(err) => {
            let status = resolve_license_status(Some(&file), &device_id, now, false);
            Ok(HeartbeatResult {
                success: false,
                status: status.kind,
                next_heartbeat_at: file.last_heartbeat.map(|last| last + HEARTBEAT_INTERVAL_SECS),
                message: Some(err.message),
            })
        }
    }
}

#[tauri::command]
pub async fn revoke_this_device(app: AppHandle) -> Result<(), String> {
    let device_id = compute_device_id()?;
    let app_data_dir = app_data_dir(&app)?;
    let mut file = load_license_file(&app_data_dir)?.ok_or_else(|| {
        "No license file found".to_string()
    })?;

    if !is_dev_mode() {
        if let Some(machine_id) = &file.machine_id {
            let config = build_keygen_config(&file.key).map_err(|e| e.message)?;
            let _ = config::set_config(config.clone());
            let client = reqwest::Client::new();
            let url = format!("{}/machines/{}", config.api_url.trim_end_matches('/'), machine_id);
            let response = client
                .delete(url)
                .header("Authorization", format!("License {}", file.key))
                .header("Accept", "application/vnd.api+json")
                .send()
                .await
                .map_err(|e| format!("Failed to revoke device: {e}"))?;
            if !response.status().is_success() && response.status() != reqwest::StatusCode::NOT_FOUND
            {
                return Err(format!(
                    "Failed to revoke device: HTTP {}",
                    response.status()
                ));
            }
        }
    }

    file.revoked = true;
    save_license_file(&app_data_dir, &file)?;
    let _ = device_id;
    Ok(())
}

#[tauri::command]
pub async fn open_in_browser(url: String, app: AppHandle) -> Result<(), String> {
    app.shell()
        .open(url, None)
        .map_err(|e| format!("Failed to open browser: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_device_id_is_stable_within_process() {
        let first = match compute_device_id() {
            Ok(id) => id,
            Err(_) => {
                // WSL/CI environments may not expose drive serial metadata.
                return;
            }
        };
        let second = compute_device_id().expect("device id should succeed after first success");
        assert_eq!(first, second);
        assert!(!first.is_empty());
    }

    #[test]
    fn parse_activate_key_extracts_license_key() {
        let key = parse_activate_key("avrag-desktop://activate?key=AVRG-TEST-KEY").expect("key");
        assert_eq!(key, "AVRG-TEST-KEY");
    }

    #[test]
    fn resolve_status_unactivated_without_file() {
        let status = resolve_license_status(None, "device-a", 1_700_000_000, true);
        assert_eq!(status.kind, LicenseStatusKind::Unactivated);
    }

    #[test]
    fn resolve_status_trial_with_remaining_days() {
        let now = 1_700_000_000_i64;
        let file = LicenseFile {
            key: "trial-key".to_string(),
            license_id: "lic-1".to_string(),
            device_id: "device-a".to_string(),
            machine_id: None,
            certificate: build_dev_certificate("device-a", Some(now + 86_400), 1),
            kind: LicenseKind::Trial,
            issued_at: now,
            expires_at: Some(now + 86_400),
            last_heartbeat: Some(now),
            revoked: false,
        };

        let status = resolve_license_status(Some(&file), "device-a", now, true);
        assert_eq!(status.kind, LicenseStatusKind::Trial);
        assert_eq!(status.days_remaining, Some(1));
    }

    #[test]
    fn resolve_status_expired_after_offline_grace() {
        let now = 1_700_000_000_i64;
        let expired_at = now - 10;
        let file = LicenseFile {
            key: "paid-key".to_string(),
            license_id: "lic-2".to_string(),
            device_id: "device-a".to_string(),
            machine_id: None,
            certificate: build_dev_certificate("device-a", Some(expired_at), 1),
            kind: LicenseKind::Standard,
            issued_at: now - OFFLINE_GRACE_SECS - 10,
            expires_at: Some(expired_at),
            last_heartbeat: Some(now - OFFLINE_GRACE_SECS - 10),
            revoked: false,
        };

        let status = resolve_license_status(Some(&file), "device-a", now, true);
        assert_eq!(status.kind, LicenseStatusKind::Expired);
    }

    #[test]
    fn resolve_status_offline_grace_before_expiry_window_ends() {
        let now = 1_700_000_000_i64;
        let expired_at = now - 10;
        let last_heartbeat = now - 86_400;
        let file = LicenseFile {
            key: "paid-key".to_string(),
            license_id: "lic-3".to_string(),
            device_id: "device-a".to_string(),
            machine_id: None,
            certificate: build_dev_certificate("device-a", Some(expired_at), 1),
            kind: LicenseKind::Standard,
            issued_at: now - 86_400,
            expires_at: Some(expired_at),
            last_heartbeat: Some(last_heartbeat),
            revoked: false,
        };

        let status = resolve_license_status(Some(&file), "device-a", now, true);
        assert_eq!(status.kind, LicenseStatusKind::OfflineGrace);
        assert!(status.offline_grace_days.unwrap_or(0) > 0);
    }

    #[test]
    fn resolve_status_revoked_flag() {
        let now = 1_700_000_000_i64;
        let file = LicenseFile {
            key: "paid-key".to_string(),
            license_id: "lic-4".to_string(),
            device_id: "device-a".to_string(),
            machine_id: None,
            certificate: build_dev_certificate("device-a", None, 1),
            kind: LicenseKind::Pro,
            issued_at: now,
            expires_at: None,
            last_heartbeat: Some(now),
            revoked: true,
        };

        let status = resolve_license_status(Some(&file), "device-a", now, true);
        assert_eq!(status.kind, LicenseStatusKind::Revoked);
    }
}
