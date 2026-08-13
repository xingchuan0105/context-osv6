use std::sync::atomic::{AtomicBool, Ordering};

use contracts::chat::ChatEvent;
use tauri::{AppHandle, Emitter, State};

use super::api::IpcApiError;
use super::chat::{
    chat_event_channel, error_events, parse_chat_request_id, session_id_from_request,
    LICENSE_REQUIRED,
};
use super::license::{get_license_status, license_allows_chat};
use super::local_product::product_api_base_url;
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
    token: String,
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

    // PR-4: desktop chat runs through the local avrag-api `conversation().execute_stream`
    // (Lead + Workers) instead of the legacy single-`complete` path. The WebView cannot
    // fetch `127.0.0.1:18080` directly (PNA), so this command proxies the SSE stream.
    let result = proxy_chat_to_local_api(&request, &token, &cancel, |event| {
        emit_or_stop(&app, event)
    })
    .await;

    registry.remove(&request_id);
    result
}

fn upstream_error_message(status: u16, text: &str) -> String {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(msg) = value
            .get("message")
            .or_else(|| value.get("error"))
            .and_then(|m| m.as_str())
        {
            return msg.to_string();
        }
    }
    if text.trim().is_empty() {
        format!("Chat failed (HTTP {status})")
    } else {
        text.to_string()
    }
}

/// Stream the local product `/api/v1/chat` SSE response into the IPC event channel.
async fn proxy_chat_to_local_api<F>(
    request: &serde_json::Value,
    token: &str,
    cancel: &AtomicBool,
    mut emit: F,
) -> Result<(), IpcApiError>
where
    F: FnMut(&ChatEvent) -> Result<bool, IpcApiError>,
{
    let base = product_api_base_url();
    let url = format!("{base}/api/v1/chat");

    let mut body = request.clone();
    if let Some(obj) = body.as_object_mut() {
        obj.insert("stream".to_string(), serde_json::json!(true));
    }

    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| IpcApiError::internal(format!("http client: {e}")))?;

    let mut req = client
        .post(&url)
        .header(reqwest::header::ACCEPT, "text/event-stream")
        .header(reqwest::header::CONTENT_TYPE, "application/json");
    if !token.trim().is_empty() {
        req = req.bearer_auth(token.trim());
    }
    req = req.json(&body);

    let resp = req.send().await.map_err(|e| {
        if e.is_connect() {
            IpcApiError::service_unavailable(format!(
                "Local product API not reachable at {base} ({e})"
            ))
        } else {
            IpcApiError::internal(format!("chat request to {url} failed: {e}"))
        }
    })?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        let message = upstream_error_message(status.as_u16(), &text);
        let session_id = session_id_from_request(request);
        let request_id = parse_chat_request_id(request).map_err(IpcApiError::from)?;
        for event in error_events(&request_id, &session_id, &message) {
            if !emit(&event)? {
                return Ok(());
            }
        }
        return Ok(());
    }

    let mut resp = resp;
    let mut buf = String::new();
    let mut event_name = String::new();
    let mut data_lines: Vec<String> = Vec::new();

    loop {
        if is_cancelled(cancel) {
            return Ok(());
        }
        match resp.chunk().await {
            Ok(Some(chunk)) => buf.push_str(&String::from_utf8_lossy(&chunk)),
            Ok(None) => break,
            Err(e) => {
                return Err(IpcApiError::internal(format!(
                    "chat stream read failed: {e}"
                )));
            }
        }

        loop {
            let Some(nl) = buf.find('\n') else { break };
            let mut line = buf[..nl].to_string();
            buf.drain(..=nl);
            if line.ends_with('\r') {
                line.pop();
            }

            if line.is_empty() {
                if data_lines.is_empty() {
                    event_name.clear();
                    continue;
                }
                let data = data_lines.join("\n");
                data_lines.clear();
                event_name.clear();
                match serde_json::from_str::<ChatEvent>(&data) {
                    Ok(event) => {
                        if !emit(&event)? {
                            return Ok(());
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "ignoring non-ChatEvent SSE frame");
                    }
                }
                continue;
            }
            if line.starts_with(':') {
                continue;
            }
            if let Some((field, value)) = line.split_once(':') {
                let value = value.strip_prefix(' ').unwrap_or(value);
                match field {
                    "event" => event_name = value.to_string(),
                    "data" => data_lines.push(value.to_string()),
                    _ => {}
                }
            }
        }
    }

    if !data_lines.is_empty() {
        let data = data_lines.join("\n");
        if let Ok(event) = serde_json::from_str::<ChatEvent>(&data) {
            let _ = emit(&event)?;
        }
    }

    Ok(())
}

#[tauri::command]
pub fn chat_cancel(
    request_id: String,
    registry: State<'_, ChatStreamRegistry>,
) -> Result<(), IpcApiError> {
    registry.cancel(&request_id);
    Ok(())
}
