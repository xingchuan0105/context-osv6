//! Tauri IPC commands for desktop licensing.
//!
//! Command errors use [`IpcApiError`] so the frontend always sees
//! `{ status, code, message }`. Domain [`LicenseError`] converts via `From`.

use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

use super::service::*;
use super::types::*;
use crate::commands::api::IpcApiError;

#[tauri::command]
pub fn get_device_id() -> Result<String, IpcApiError> {
    compute_device_id().map_err(IpcApiError::from)
}

#[tauri::command]
pub async fn start_trial(app: AppHandle) -> Result<TrialResult, IpcApiError> {
    let device_id =
        compute_device_id().map_err(|e| LicenseError::new("device_id", e))?;
    let app_data_dir =
        app_data_dir(&app).map_err(|e| LicenseError::new("app_data_dir", e))?;

    if let Some(existing) = load_license_file(&app_data_dir)
        .map_err(|e| LicenseError::new("license_file", e))?
    {
        if existing.device_id == device_id && !existing.revoked {
            let status =
                resolve_license_status(Some(&existing), &device_id, now_unix(), is_dev_mode());
            if status.kind != LicenseStatusKind::Unactivated {
                return Err(LicenseError::new(
                    "trial_already_used",
                    "This device already has a license or trial",
                )
                .into());
            }
        }
    }

    // ADR-0010 free client: optional local trial marker only (not a product gate).
    let expires_at = now_unix() + TRIAL_DURATION_SECS;
    mock_activate(
        &format!("LOCAL-TRIAL-{}", uuid::Uuid::new_v4()),
        &device_id,
        &app_data_dir,
        LicenseKind::Trial,
        Some(expires_at),
    )
    .await?;
    Ok(TrialResult {
        expires_at,
        days_remaining: TRIAL_DURATION_DAYS as i32,
    })
}

#[tauri::command]
pub async fn activate_license(
    license_key: String,
    app: AppHandle,
) -> Result<ActivationResult, IpcApiError> {
    let device_id =
        compute_device_id().map_err(|e| LicenseError::new("device_id", e))?;
    let app_data_dir =
        app_data_dir(&app).map_err(|e| LicenseError::new("app_data_dir", e))?;

    // ADR-0010: no Keygen — store a local free-active marker if user pastes a legacy key.
    activate_with_keygen(&license_key, &device_id, &app_data_dir)
        .await
        .map_err(IpcApiError::from)
}

#[tauri::command]
pub async fn get_license_status(app: AppHandle) -> Result<LicenseStatus, IpcApiError> {
    let device_id = compute_device_id().map_err(IpcApiError::from)?;
    let app_data_dir = app_data_dir(&app).map_err(IpcApiError::from)?;
    let file = load_license_file(&app_data_dir).map_err(IpcApiError::from)?;
    Ok(resolve_license_status(
        file.as_ref(),
        &device_id,
        now_unix(),
        is_dev_mode(),
    ))
}

#[tauri::command]
pub async fn heartbeat_license(app: AppHandle) -> Result<HeartbeatResult, IpcApiError> {
    let device_id = compute_device_id().map_err(IpcApiError::from)?;
    let app_data_dir = app_data_dir(&app).map_err(IpcApiError::from)?;
    let mut file = load_license_file(&app_data_dir)
        .map_err(IpcApiError::from)?
        .ok_or_else(|| IpcApiError::not_found("No license file found"))?;

    let now = now_unix();
    if file
        .last_heartbeat
        .is_some_and(|last| now - last < HEARTBEAT_INTERVAL_SECS)
    {
        let status = resolve_license_status(Some(&file), &device_id, now, is_dev_mode());
        return Ok(HeartbeatResult {
            success: true,
            status: status.kind,
            next_heartbeat_at: file
                .last_heartbeat
                .map(|last| last + HEARTBEAT_INTERVAL_SECS),
            message: Some("Heartbeat skipped; interval not reached".to_string()),
        });
    }

    // ADR-0010: local heartbeat only (Keygen remote path removed).
    file.last_heartbeat = Some(now);
    save_license_file(&app_data_dir, &file).map_err(IpcApiError::from)?;
    let status = resolve_license_status(Some(&file), &device_id, now, true);
    Ok(HeartbeatResult {
        success: true,
        status: status.kind,
        next_heartbeat_at: Some(now + HEARTBEAT_INTERVAL_SECS),
        message: Some("Local free-client heartbeat recorded".to_string()),
    })
}

#[tauri::command]
pub async fn revoke_this_device(app: AppHandle) -> Result<(), IpcApiError> {
    let device_id = compute_device_id().map_err(IpcApiError::from)?;
    let app_data_dir = app_data_dir(&app).map_err(IpcApiError::from)?;
    let mut file = load_license_file(&app_data_dir)
        .map_err(IpcApiError::from)?
        .ok_or_else(|| IpcApiError::not_found("No license file found"))?;

    // ADR-0010: local revoke only (no Keygen machine API).
    file.revoked = true;
    save_license_file(&app_data_dir, &file).map_err(IpcApiError::from)?;
    let _ = device_id;
    Ok(())
}

/// Open URL in the OS default browser.
///
/// Prefer a direct OS launch on Windows so a missing shell ACL / plugin path
/// cannot leave the UI to fall back to `window.open` (which triggers WebView2
/// NewWindowRequested and has been observed to abort the process under
/// `x86_64-pc-windows-gnu` COM callbacks).
#[tauri::command]
pub async fn open_in_browser(url: String, app: AppHandle) -> Result<(), IpcApiError> {
    if !(url.starts_with("https://")
        || url.starts_with("http://")
        || url.starts_with("mailto:"))
    {
        return Err(IpcApiError::internal(format!(
            "Refusing to open non-http(s) URL: {url}"
        )));
    }

    // 1) Direct OS open (no plugin ACL dependency).
    if crate::commands::system::open_with_os(std::ffi::OsStr::new(&url)).is_ok() {
        return Ok(());
    }

    // 2) Shell plugin fallback.
    #[allow(deprecated)]
    {
        if app.shell().open(&url, None).is_ok() {
            return Ok(());
        }
    }

    Err(IpcApiError::internal(format!(
        "Failed to open browser for {url}"
    )))
}

