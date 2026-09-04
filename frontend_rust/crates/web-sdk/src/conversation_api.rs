use crate::transport::TransportError;
use contracts::chat::ChatMessageListResponse;
use contracts::workspaces::{ChatSession, ChatSessionListResponse};

pub fn trim_base_url(base_url: &str) -> &str {
    base_url.trim_end_matches('/')
}

/// RFC 3986 unreserved characters stay literal; everything else is percent-encoded.
pub fn encode_path_segment(raw: &str) -> String {
    let mut out = String::new();
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

pub fn sessions_url(base_url: &str) -> String {
    format!("{}/api/v1/chat/sessions", trim_base_url(base_url))
}

pub fn session_url(base_url: &str, session_id: &str) -> String {
    format!(
        "{}/api/v1/chat/sessions/{}",
        trim_base_url(base_url),
        encode_path_segment(session_id)
    )
}

pub fn session_messages_url(base_url: &str, session_id: &str) -> String {
    format!(
        "{}/api/v1/chat/sessions/{}/messages",
        trim_base_url(base_url),
        encode_path_segment(session_id)
    )
}

pub fn parse_session_list(body: &[u8]) -> Result<ChatSessionListResponse, TransportError> {
    serde_json::from_slice(body).map_err(TransportError::from)
}

pub fn parse_session(body: &[u8]) -> Result<ChatSession, TransportError> {
    serde_json::from_slice(body).map_err(TransportError::from)
}

pub fn parse_message_list(body: &[u8]) -> Result<ChatMessageListResponse, TransportError> {
    serde_json::from_slice(body).map_err(TransportError::from)
}
