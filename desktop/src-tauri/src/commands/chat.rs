use std::sync::atomic::AtomicBool;

use avrag_llm::{ChatMessage, LlmClient};
use contracts::chat::ChatEvent;
use tauri::{AppHandle, Manager};

use super::llm_config::{load_llm_config, LocalLlmConfig};
use crate::commands::api::IpcApiError;

const LLM_NOT_CONFIGURED: &str =
    "LLM is not configured. Open Settings → AI Model to add your API key.";
pub const LICENSE_REQUIRED: &str = "License required. Please activate AVRag Desktop first.";

pub fn chat_event_channel(request_id: &str) -> String {
    format!("chat://{request_id}")
}

pub fn session_id_from_request(request: &serde_json::Value) -> String {
    request
        .get("session_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}

pub fn parse_chat_request_id(request: &serde_json::Value) -> Result<String, IpcApiError> {
    request
        .get("request_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| IpcApiError::bad_request("invalid_request", "request_id is required"))
}

pub fn query_from_request(request: &serde_json::Value) -> Result<String, IpcApiError> {
    request
        .get("query")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| IpcApiError::bad_request("invalid_request", "query is required"))
}

pub fn error_events(request_id: &str, session_id: &str, message: &str) -> Vec<ChatEvent> {
    let message_id: i64 = 1;
    vec![
        ChatEvent::Start {
            request_id: request_id.to_string(),
            session_id: session_id.to_string(),
        },
        ChatEvent::AnswerStart {
            request_id: request_id.to_string(),
            session_id: session_id.to_string(),
            message_id,
            agent_type: "chat".to_string(),
        },
        ChatEvent::Error {
            request_id: request_id.to_string(),
            code: "desktop_error".to_string(),
            message: message.to_string(),
        },
        ChatEvent::Done {
            request_id: request_id.to_string(),
            session_id: session_id.to_string(),
            message_id,
            payload: serde_json::json!({
                "answer": message,
                "status": "error",
            }),
        },
    ]
}

pub async fn run_desktop_chat<F>(
    app: &AppHandle,
    request: &serde_json::Value,
    cancel: &AtomicBool,
    mut emit: F,
) -> Result<(), IpcApiError>
where
    F: FnMut(&ChatEvent) -> Result<bool, IpcApiError>,
{
    let request_id = parse_chat_request_id(request)?;
    let session_id = session_id_from_request(request);
    let query = query_from_request(request)?;

    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| IpcApiError::internal(format!("Failed to get app data dir: {e}")))?;

    let Some(config) = load_llm_config(&data_dir).map_err(IpcApiError::from)? else {
        for event in error_events(&request_id, &session_id, LLM_NOT_CONFIGURED) {
            if !emit(&event)? {
                return Ok(());
            }
        }
        return Ok(());
    };

    stream_llm_response(&request_id, &session_id, &query, &config, cancel, emit).await
}

async fn stream_llm_response<F>(
    request_id: &str,
    session_id: &str,
    query: &str,
    config: &LocalLlmConfig,
    _cancel: &AtomicBool,
    mut emit: F,
) -> Result<(), IpcApiError>
where
    F: FnMut(&ChatEvent) -> Result<bool, IpcApiError>,
{
    let message_id: i64 = 1;
    let start = ChatEvent::Start {
        request_id: request_id.to_string(),
        session_id: session_id.to_string(),
    };
    if !emit(&start)? {
        return Ok(());
    }

    let answer_start = ChatEvent::AnswerStart {
        request_id: request_id.to_string(),
        session_id: session_id.to_string(),
        message_id,
        agent_type: "chat".to_string(),
    };
    if !emit(&answer_start)? {
        return Ok(());
    }

    let client = LlmClient::new(config.to_provider());
    let messages = vec![ChatMessage::user(query)];

    let response = client
        .complete(&messages, Some(0.7))
        .await
        .map_err(|e| IpcApiError::internal(format!("LLM request failed: {e}")))?;

    let answer = response.content.clone();
    if !answer.is_empty() {
        let event = ChatEvent::Token {
            request_id: request_id.to_string(),
            message_id,
            content: answer.clone(),
        };
        if !emit(&event)? {
            return Ok(());
        }
    }

    let done = ChatEvent::Done {
        request_id: request_id.to_string(),
        session_id: session_id.to_string(),
        message_id,
        payload: serde_json::json!({
            "answer": answer,
            "status": "done",
        }),
    };
    let _ = emit(&done)?;
    Ok(())
}

pub fn desktop_placeholder_events(request_id: &str, session_id: &str) -> Vec<ChatEvent> {
    error_events(
        request_id,
        session_id,
        "[Desktop mode] Chat is not yet connected to LLM backend. This is a placeholder response.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn chat_event_channel_uses_request_scoped_prefix() {
        assert_eq!(chat_event_channel("req-42"), "chat://req-42");
    }

    #[test]
    fn query_from_request_requires_non_empty_value() {
        let err = query_from_request(&json!({})).unwrap_err();
        assert_eq!(err.code, "invalid_request");
        assert_eq!(err.message, "query is required");
    }

    #[test]
    fn error_events_match_frontend_stream_contract() {
        let events = error_events("req-ipc", "sess-ipc", "boom");
        assert_eq!(events.len(), 4);
        assert!(matches!(events[2], ChatEvent::Error { .. }));
    }
}
