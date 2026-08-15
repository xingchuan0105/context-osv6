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

/// Open a target (URL or directory) with the OS default handler — shared by
/// `license::commands::open_in_browser` and the drawer dir buttons. Direct OS
/// launch only: no plugin ACL in the path.
pub(crate) fn open_with_os(target: &std::ffi::OsStr) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        // `cmd /C start "" <target>`; the empty title avoids swallowing a
        // quoted target. Avoid `explorer.exe <url>` which sometimes reuses a
        // shell tab.
        let mut cmd = std::process::Command::new("cmd");
        cmd.args(["/C", "start", ""]).arg(target);
        crate::commands::win_cmd::hide_console(&mut cmd);
        cmd.spawn().map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(target)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(target)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
    {
        let _ = target;
        Err("open_with_os unsupported on this OS".into())
    }
}

fn open_dir(path: std::path::PathBuf) -> Result<(), IpcApiError> {
    std::fs::create_dir_all(&path)
        .map_err(|e| IpcApiError::internal(format!("create dir {}: {e}", path.display())))?;
    open_with_os(path.as_os_str())
        .map_err(|e| IpcApiError::internal(format!("open dir {}: {e}", path.display())))
}

/// App data dir (cloud_session.json, client state) in the OS file manager.
#[tauri::command]
pub async fn open_data_dir(app: tauri::AppHandle) -> Result<(), IpcApiError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| IpcApiError::internal(format!("Failed to get app data dir: {e}")))?;
    open_dir(dir)
}

/// Local product logs dir (api.log / worker.log) in the OS file manager.
#[tauri::command]
pub async fn open_logs_dir() -> Result<(), IpcApiError> {
    open_dir(crate::commands::local_product::log_dir_path())
}
