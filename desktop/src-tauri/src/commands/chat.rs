use contracts::chat::ChatEvent;

const DESKTOP_PLACEHOLDER: &str =
    "[Desktop mode] Chat is not yet connected to LLM backend. This is a placeholder response.";

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

pub fn parse_chat_request_id(request: &serde_json::Value) -> Result<String, String> {
    request
        .get("request_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "request_id is required".to_string())
}

pub fn desktop_placeholder_events(request_id: &str, session_id: &str) -> Vec<ChatEvent> {
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
        ChatEvent::Token {
            request_id: request_id.to_string(),
            message_id,
            content: DESKTOP_PLACEHOLDER.to_string(),
        },
        ChatEvent::Done {
            request_id: request_id.to_string(),
            session_id: session_id.to_string(),
            message_id,
            payload: serde_json::json!({
                "answer": DESKTOP_PLACEHOLDER,
                "status": "done",
            }),
        },
    ]
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
    fn session_id_from_request_uses_provided_value() {
        let request = json!({ "session_id": "sess-1" });
        assert_eq!(session_id_from_request(&request), "sess-1");
    }

    #[test]
    fn session_id_from_request_generates_uuid_when_missing() {
        let request = json!({ "query": "hello" });
        assert!(!session_id_from_request(&request).is_empty());
    }

    #[test]
    fn session_id_from_request_generates_uuid_when_empty() {
        let request = json!({ "session_id": "" });
        assert!(!session_id_from_request(&request).is_empty());
    }

    #[test]
    fn parse_chat_request_id_requires_non_empty_value() {
        assert_eq!(
            parse_chat_request_id(&json!({})).unwrap_err(),
            "request_id is required"
        );
        assert_eq!(
            parse_chat_request_id(&json!({ "request_id": "" })).unwrap_err(),
            "request_id is required"
        );
    }

    #[test]
    fn parse_chat_request_id_returns_trimmed_request_id() {
        assert_eq!(
            parse_chat_request_id(&json!({ "request_id": "req-ipc" })).unwrap(),
            "req-ipc"
        );
    }

    #[test]
    fn desktop_placeholder_events_match_frontend_stream_contract() {
        let events = desktop_placeholder_events("req-ipc", "sess-ipc");
        assert_eq!(events.len(), 4);

        let serialized: Vec<serde_json::Value> = events
            .iter()
            .map(|event| serde_json::to_value(event).expect("serialize chat event"))
            .collect();

        assert_eq!(serialized[0]["event"], "start");
        assert_eq!(serialized[0]["request_id"], "req-ipc");
        assert_eq!(serialized[0]["session_id"], "sess-ipc");

        assert_eq!(serialized[1]["event"], "answer_start");
        assert_eq!(serialized[1]["agent_type"], "chat");
        assert_eq!(serialized[1]["message_id"], 1);

        assert_eq!(serialized[2]["event"], "token");
        assert_eq!(serialized[2]["content"], DESKTOP_PLACEHOLDER);

        assert_eq!(serialized[3]["event"], "done");
        assert_eq!(serialized[3]["payload"]["status"], "done");
        assert_eq!(serialized[3]["payload"]["answer"], DESKTOP_PLACEHOLDER);
    }
}
