use contracts::chat::ChatEvent;

use crate::commands::api::IpcApiError;

pub const LICENSE_REQUIRED: &str =
    "License required. Please activate Context-OS first.";

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_event_channel_uses_request_scoped_prefix() {
        assert_eq!(chat_event_channel("req-42"), "chat://req-42");
    }

    #[test]
    fn error_events_match_frontend_stream_contract() {
        let events = error_events("req-ipc", "sess-ipc", "boom");
        assert_eq!(events.len(), 4);
        assert!(matches!(events[2], ChatEvent::Error { .. }));
    }
}
