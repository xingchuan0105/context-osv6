use contracts::chat::ChatEvent;

use crate::commands::api::IpcApiError;

pub const LICENSE_REQUIRED: &str =
    "License required. Please activate Context-OS first.";

pub fn chat_event_channel(request_id: &str) -> String {
    format!("chat://{request_id}")
}

pub fn parse_chat_request_id(request: &serde_json::Value) -> Result<String, IpcApiError> {
    request
        .get("request_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| IpcApiError::bad_request("invalid_request", "request_id is required"))
}

pub fn pre_start_error_event(request_id: &str, message: &str) -> ChatEvent {
    ChatEvent::Error {
        request_id: request_id.to_string(),
        code: "desktop_error".to_string(),
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_event_channel_uses_request_scoped_prefix() {
        assert_eq!(chat_event_channel("req-42"), "chat://req-42");
    }

    #[test]
    fn pre_start_failure_is_one_terminal_error_without_session_id() {
        let event = pre_start_error_event("req-ipc", "boom");
        assert_eq!(
            event,
            ChatEvent::Error {
                request_id: "req-ipc".to_string(),
                code: "desktop_error".to_string(),
                message: "boom".to_string(),
            }
        );

        let wire = serde_json::to_value(event).expect("serialize error event");
        assert_eq!(wire["event"], "error");
        assert!(wire.get("session_id").is_none());
    }
}
