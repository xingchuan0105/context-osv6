//! License file I/O, verification, status resolution, and Keygen calls.
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use keygen_rs::config::{self, KeygenConfig};
use keygen_rs::errors::Error as KeygenError;
use keygen_rs::license::SchemeCode;
use machineid_rs::{Encryption, HWIDComponent, IdBuilder};
use tauri::{AppHandle, Emitter, Manager};
use url::Url;

use super::types::*;

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

pub fn license_path(app_data_dir: &Path) -> PathBuf {
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

pub(crate) fn build_dev_certificate(
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

pub(crate) fn build_keygen_config(license_key: &str) -> Result<KeygenConfig, LicenseError> {
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

pub(crate) async fn keygen_validate_and_activate(
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

pub(crate) async fn activate_with_keygen(
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

pub(crate) async fn mock_activate(
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

pub(crate) async fn request_trial_license(device_id: &str) -> Result<String, LicenseError> {
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

pub(crate) fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {e}"))
}

