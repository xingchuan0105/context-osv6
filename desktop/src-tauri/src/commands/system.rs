use tauri::Manager;

use crate::commands::api::IpcApiError;

#[tauri::command]
pub fn get_app_data_dir(app: tauri::AppHandle) -> Result<String, IpcApiError> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| IpcApiError::internal(format!("Failed to get app data dir: {e}")))?;

    Ok(data_dir.to_string_lossy().to_string())
}

#[tauri::command]
pub fn is_tauri_environment() -> bool {
    true
}

#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
