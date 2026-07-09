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
) -> Result<ActivationResult, IpcApiError> {
    let device_id =
        compute_device_id().map_err(|e| LicenseError::new("device_id", e))?;
    let app_data_dir =
        app_data_dir(&app).map_err(|e| LicenseError::new("app_data_dir", e))?;

    if is_dev_mode() {
        return mock_activate(
            &license_key,
            &device_id,
            &app_data_dir,
            LicenseKind::Standard,
            None,
        )
        .await
        .map_err(IpcApiError::from);
    }

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

    if is_dev_mode() {
        file.last_heartbeat = Some(now);
        save_license_file(&app_data_dir, &file).map_err(IpcApiError::from)?;
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
            save_license_file(&app_data_dir, &file).map_err(IpcApiError::from)?;
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
                next_heartbeat_at: file
                    .last_heartbeat
                    .map(|last| last + HEARTBEAT_INTERVAL_SECS),
                message: Some(err.message),
            })
        }
    }
}

#[tauri::command]
pub async fn revoke_this_device(app: AppHandle) -> Result<(), IpcApiError> {
    let device_id = compute_device_id().map_err(IpcApiError::from)?;
    let app_data_dir = app_data_dir(&app).map_err(IpcApiError::from)?;
    let mut file = load_license_file(&app_data_dir)
        .map_err(IpcApiError::from)?
        .ok_or_else(|| IpcApiError::not_found("No license file found"))?;

    if !is_dev_mode() {
        if let Some(machine_id) = &file.machine_id {
            let config = build_keygen_config(&file.key).map_err(IpcApiError::from)?;
            let _ = keygen_rs::config::set_config(config.clone());
            let client = reqwest::Client::new();
            let url = format!(
                "{}/machines/{}",
                config.api_url.trim_end_matches('/'),
                machine_id
            );
            let response = client
                .delete(url)
                .header("Authorization", format!("License {}", file.key))
                .header("Accept", "application/vnd.api+json")
                .send()
                .await
                .map_err(|e| IpcApiError::internal(format!("Failed to revoke device: {e}")))?;
            if !response.status().is_success() && response.status() != reqwest::StatusCode::NOT_FOUND
            {
                return Err(IpcApiError::internal(format!(
                    "Failed to revoke device: HTTP {}",
                    response.status()
                )));
            }
        }
    }

    file.revoked = true;
    save_license_file(&app_data_dir, &file).map_err(IpcApiError::from)?;
    let _ = device_id;
    Ok(())
}

#[tauri::command]
pub async fn open_in_browser(url: String, app: AppHandle) -> Result<(), IpcApiError> {
    app.shell()
        .open(url, None)
        .map_err(|e| IpcApiError::internal(format!("Failed to open browser: {e}")))
}
