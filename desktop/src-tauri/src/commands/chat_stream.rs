use std::sync::atomic::{AtomicBool, Ordering};

use contracts::chat::ChatEvent;
use tauri::{AppHandle, Emitter, State};

use super::chat::{
    chat_event_channel, error_events, parse_chat_request_id, run_desktop_chat,
    session_id_from_request, LICENSE_REQUIRED,
};
use super::license::{get_license_status, license_allows_chat};
use crate::commands::api::IpcApiError;
use crate::registry::ChatStreamRegistry;

fn emit_chat_event(app: &AppHandle, request_id: &str, event: &ChatEvent) -> Result<(), IpcApiError> {
    app.emit(&chat_event_channel(request_id), event)
        .map_err(|e| IpcApiError::internal(format!("Failed to emit chat event: {e}")))
}

fn is_cancelled(cancel: &AtomicBool) -> bool {
    cancel.load(Ordering::SeqCst)
}

#[tauri::command]
pub async fn chat_stream(
    _token: String,
    request: serde_json::Value,
    app: tauri::AppHandle,
    registry: State<'_, ChatStreamRegistry>,
) -> Result<(), IpcApiError> {
    let request_id = parse_chat_request_id(&request).map_err(IpcApiError::from)?;
    let session_id = session_id_from_request(&request);
    let cancel = registry.register(&request_id);

    let license_status = get_license_status(app.clone()).await?;
    if !license_allows_chat(license_status.kind) {
        let emit_or_stop = |app: &AppHandle, event: &ChatEvent| -> Result<bool, IpcApiError> {
            if is_cancelled(&cancel) {
                return Ok(false);
            }
            emit_chat_event(app, &request_id, event)?;
            Ok(true)
        };
        for event in error_events(&request_id, &session_id, LICENSE_REQUIRED) {
            if !emit_or_stop(&app, &event)? {
                break;
            }
        }
        registry.remove(&request_id);
        return Ok(());
    }

    let emit_or_stop = |app: &AppHandle, event: &ChatEvent| -> Result<bool, IpcApiError> {
        if is_cancelled(&cancel) {
            return Ok(false);
        }
        emit_chat_event(app, &request_id, event)?;
        Ok(true)
    };

    let result = run_desktop_chat(&app, &request, &cancel, |event| emit_or_stop(&app, event))
        .await
        .map_err(IpcApiError::from);

    registry.remove(&request_id);
    result
}

#[tauri::command]
pub fn chat_cancel(
    request_id: String,
    registry: State<'_, ChatStreamRegistry>,
) -> Result<(), IpcApiError> {
    registry.cancel(&request_id);
    Ok(())
}
