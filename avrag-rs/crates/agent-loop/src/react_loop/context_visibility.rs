//! LLM-boundary transform: collapse old retrieval observations (P1′).
//!
//! Durable `state.messages` can keep full history for tooling; the copy fed to
//! the model stubs older retrieval payloads so attention is not diluted.

use avrag_llm::ChatMessage;
use serde_json::Value;

/// Keep this many most-recent retrieval-bearing user/tool messages fully expanded.
pub const HISTORY_FULL_RETRIEVAL_ROUNDS: usize = 2;
/// Soft cap for expanded chunk text retained across the working set (chars).
/// Reserved for token-based working-set accounting (U-P2 extension).
#[allow(dead_code)]
pub const WORKING_SET_CHAR_BUDGET: usize = 16_000;

fn looks_like_retrieval_payload(s: &str) -> bool {
    s.contains("chunk_id") && (s.contains("\"text\"") || s.contains("\"content\"") || s.contains("body_omitted"))
}

/// Collapse chunk bodies in a JSON value tree (in place).
fn strip_chunk_bodies(v: &mut Value) {
    match v {
        Value::Array(arr) => {
            for item in arr {
                strip_chunk_bodies(item);
            }
        }
        Value::Object(map) => {
            let is_chunk = map.contains_key("chunk_id")
                && (map.contains_key("text") || map.contains_key("content") || map.contains_key("alias"));
            if is_chunk {
                if let Some(t) = map.get_mut("text") {
                    *t = Value::String(String::new());
                }
                if let Some(t) = map.get_mut("content") {
                    *t = Value::String(String::new());
                }
                map.insert("body_omitted".into(), Value::Bool(true));
                map.insert("visibility".into(), Value::String("stub".into()));
                map.insert("history_cleared".into(), Value::Bool(true));
            } else {
                for (_k, child) in map.iter_mut() {
                    strip_chunk_bodies(child);
                }
            }
        }
        _ => {}
    }
}

fn collapse_message_content(content: &str) -> String {
    // Try whole content as JSON first (native tool messages).
    if let Ok(mut v) = serde_json::from_str::<Value>(content) {
        strip_chunk_bodies(&mut v);
        return v.to_string();
    }
    // Heuristic: strip "text":"..." / "content":"..." values inside retrieval-ish blobs.
    let mut out = content.to_string();
    for key in ["\"text\"", "\"content\""] {
        // Non-greedy-ish: replace long string values after key with empty.
        // Safe for model-visible history; durable state.messages unchanged.
        let mut result = String::with_capacity(out.len());
        let mut rest = out.as_str();
        while let Some(pos) = rest.find(key) {
            result.push_str(&rest[..pos]);
            result.push_str(key);
            let after_key = &rest[pos + key.len()..];
            let after_key = after_key.trim_start();
            if let Some(stripped) = after_key.strip_prefix(':') {
                let stripped = stripped.trim_start();
                if let Some(s) = stripped.strip_prefix('"') {
                    result.push_str(": \"\"");
                    if let Some(end) = find_json_string_end(s) {
                        rest = &s[end..];
                        continue;
                    }
                }
            }
            rest = after_key;
        }
        result.push_str(rest);
        out = result;
    }
    if out.len() + 64 < content.len() {
        out.push_str("\n[history_cleared] older retrieval bodies stubbed");
    }
    out
}

/// End index past closing quote of a JSON string body `s` (content after opening quote).
fn find_json_string_end(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i = i.saturating_add(2),
            b'"' => return Some(i + 1),
            _ => i += 1,
        }
    }
    None
}

/// Build model-facing messages: keep last `keep_recent` retrieval payloads full;
/// stub older ones. Does not mutate the source slice.
pub fn transform_messages_for_llm(messages: &[ChatMessage], keep_recent: usize) -> Vec<ChatMessage> {
    let mut retrieval_indices: Vec<usize> = Vec::new();
    for (i, m) in messages.iter().enumerate() {
        if (m.role == "user" || m.role == "tool") && looks_like_retrieval_payload(&m.content) {
            retrieval_indices.push(i);
        }
    }
    let keep_from = retrieval_indices
        .len()
        .saturating_sub(keep_recent);
    let keep_set: std::collections::HashSet<usize> = retrieval_indices
        .iter()
        .skip(keep_from)
        .copied()
        .collect();

    messages
        .iter()
        .enumerate()
        .map(|(i, m)| {
            if keep_set.contains(&i) || !looks_like_retrieval_payload(&m.content) {
                return m.clone();
            }
            let mut out = m.clone();
            out.content = collapse_message_content(&m.content);
            out
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_recent_retrieval_full() {
        let mut msgs = vec![ChatMessage::user("q")];
        for i in 0..4 {
            msgs.push(ChatMessage::user(format!(
                r#"{{"chunks":[{{"chunk_id":"{i}","text":"{}"}}]}}"#,
                "x".repeat(100)
            )));
        }
        let out = transform_messages_for_llm(&msgs, 2);
        assert!(out[1].content.contains("history_cleared") || out[1].content.contains("body_omitted") || out[1].content.len() < out[3].content.len());
        // Last two stay large
        assert!(out[3].content.contains("chunk_id"));
        assert!(out[4].content.contains("xxx") || out[4].content.len() > 50);
    }
}
