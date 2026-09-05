use std::sync::atomic::Ordering;

use contracts::chat::ChatEvent;
use tauri::{AppHandle, Emitter, State};

use super::api::IpcApiError;
use super::chat::{chat_event_channel, parse_chat_request_id, pre_start_error_event};
use super::local_product::product_api_base_url;
use crate::registry::ChatStreamRegistry;

fn emit_chat_event(app: &AppHandle, request_id: &str, event: &ChatEvent) -> Result<(), IpcApiError> {
    app.emit(&chat_event_channel(request_id), event)
        .map_err(|e| IpcApiError::internal(format!("Failed to emit chat event: {e}")))
}

#[tauri::command]
pub async fn chat_stream(
    token: String,
    request: serde_json::Value,
    app: tauri::AppHandle,
    registry: State<'_, ChatStreamRegistry>,
) -> Result<(), IpcApiError> {
    let request_id = parse_chat_request_id(&request).map_err(IpcApiError::from)?;
    let cancel = registry.register(&request_id);

    let cancel_flag = cancel.clone();
    let is_cancelled = move || cancel_flag.load(Ordering::SeqCst);

    let base = product_api_base_url();
    let req_id_clone = request_id.clone();
    let app_handle = app.clone();

    let result = desktop_core::stream_chat_sse(
        &base,
        &request,
        Some(&token),
        is_cancelled,
        move |event| {
            emit_chat_event(&app_handle, &req_id_clone, event)
                .map(|_| true)
                .map_err(|e| desktop_core::DesktopStreamError::Callback(e.to_string()))
        },
    )
    .await;

    registry.remove(&request_id);

    match result {
        Ok(()) => Ok(()),
        Err(desktop_core::DesktopStreamError::UpstreamApi { message, .. }) => {
            let event = pre_start_error_event(&request_id, &message);
            let _ = emit_chat_event(&app, &request_id, &event);
            Ok(())
        }
        Err(desktop_core::DesktopStreamError::ServiceUnavailable(msg)) => {
            Err(IpcApiError::service_unavailable(msg))
        }
        Err(e) => Err(IpcApiError::internal(e.to_string())),
    }
}

#[tauri::command]
pub fn chat_cancel(
    request_id: String,
    registry: State<'_, ChatStreamRegistry>,
) -> Result<(), IpcApiError> {
    registry.cancel(&request_id);
    Ok(())
}
