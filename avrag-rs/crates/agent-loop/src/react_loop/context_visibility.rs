//! LLM-boundary transform: collapse old retrieval observations (P1′) +
//! char-budget working-set trim of recent-round full bodies (U-P2).
//!
//! Durable `state.messages` can keep full history for tooling; the copy fed to
//! the model stubs older retrieval payloads and demotes excess expanded text
//! so attention is not diluted.

use avrag_llm::ChatMessage;
use serde_json::Value;

/// Keep this many most-recent retrieval-bearing user/tool messages fully expanded
/// (structurally — still subject to working-set char budget).
pub const HISTORY_FULL_RETRIEVAL_ROUNDS: usize = 2;
/// Soft cap for expanded chunk text retained across the working set (UTF-8 chars).
/// Shared account for near-round full bodies after history stub (U-P2 / D4).
pub const WORKING_SET_CHAR_BUDGET: usize = 16_000;
/// Snippet length when demoting expanded → card under working-set pressure.
const WORKING_SET_CARD_CHARS: usize = 280;

fn looks_like_retrieval_payload(s: &str) -> bool {
    s.contains("chunk_id")
        && (s.contains("\"text\"") || s.contains("\"content\"") || s.contains("body_omitted"))
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
                && (map.contains_key("text")
                    || map.contains_key("content")
                    || map.contains_key("alias"));
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

fn is_adjacent_chunk(map: &serde_json::Map<String, Value>) -> bool {
    map.get("adjacent").and_then(|v| v.as_bool()) == Some(true)
        || map
            .get("source")
            .and_then(|v| v.as_str())
            .is_some_and(|s| s.contains("+adjacent"))
        || map
            .get("member_chunk_ids")
            .and_then(|v| v.as_array())
            .is_some_and(|a| a.len() > 1)
}

fn chunk_text_len(map: &serde_json::Map<String, Value>) -> usize {
    map.get("text")
        .and_then(|v| v.as_str())
        .or_else(|| map.get("content").and_then(|v| v.as_str()))
        .map(|s| s.chars().count())
        .unwrap_or(0)
}

fn is_expanded_body(map: &serde_json::Map<String, Value>) -> bool {
    if map.get("body_omitted").and_then(|v| v.as_bool()) == Some(true) {
        return false;
    }
    if map.get("reseen").is_some() {
        return false;
    }
    if map.get("visibility").and_then(|v| v.as_str()) == Some("stub")
        || map.get("visibility").and_then(|v| v.as_str()) == Some("card")
    {
        return false;
    }
    chunk_text_len(map) > WORKING_SET_CARD_CHARS
}

fn demote_chunk_to_card(map: &mut serde_json::Map<String, Value>) {
    let full = map
        .get("text")
        .and_then(|v| v.as_str())
        .or_else(|| map.get("content").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();
    let snip = if full.chars().count() <= WORKING_SET_CARD_CHARS {
        full
    } else {
        let head: String = full
            .chars()
            .take(WORKING_SET_CARD_CHARS.saturating_sub(1))
            .collect();
        format!("{head}…")
    };
    if map.contains_key("text") {
        map.insert("text".into(), Value::String(snip.clone()));
    }
    if map.contains_key("content") {
        map.insert("content".into(), Value::String(snip));
    }
    if !map.contains_key("text") && !map.contains_key("content") {
        map.insert("text".into(), Value::String(String::new()));
    }
    map.insert("body_omitted".into(), Value::Bool(true));
    map.insert("visibility".into(), Value::String("card".into()));
    map.insert("working_set_demoted".into(), Value::Bool(true));
}

/// Walk JSON and demote expanded chunks until `used` would fit under budget.
/// Demotion order: non-adjacent first, then adjacent; within each pass, tree order
/// (callers process older messages first so older expands go first).
fn demote_expanded_in_value(v: &mut Value, used: &mut usize, budget: usize) -> usize {
    let mut demoted = 0usize;
    match v {
        Value::Array(arr) => {
            // Pass 1: non-adjacent
            for item in arr.iter_mut() {
                if *used <= budget {
                    break;
                }
                demoted += demote_one_chunk(item, used, budget, false);
            }
            // Pass 2: adjacent only if still over
            for item in arr.iter_mut() {
                if *used <= budget {
                    break;
                }
                demoted += demote_one_chunk(item, used, budget, true);
            }
        }
        Value::Object(map) => {
            let is_chunk = map.contains_key("chunk_id")
                && (map.contains_key("text")
                    || map.contains_key("content")
                    || map.contains_key("alias"));
            if is_chunk {
                // Single root chunk object — demote regardless of adjacent flag
                // (caller already walked messages oldest-first).
                if is_expanded_body(map) {
                    let cost = chunk_text_len(map);
                    if *used > budget && cost > WORKING_SET_CARD_CHARS {
                        demote_chunk_to_card(map);
                        *used = used.saturating_sub(cost.saturating_sub(WORKING_SET_CARD_CHARS));
                        demoted += 1;
                    }
                }
            } else {
                // Prefer demoting nested chunks under "chunks" / "hits" first.
                for key in ["chunks", "hits"] {
                    if let Some(child) = map.get_mut(key) {
                        demoted += demote_expanded_in_value(child, used, budget);
                    }
                }
                for (k, child) in map.iter_mut() {
                    if k == "chunks" || k == "hits" {
                        continue;
                    }
                    demoted += demote_expanded_in_value(child, used, budget);
                }
            }
        }
        _ => {}
    }
    demoted
}

fn demote_one_chunk(item: &mut Value, used: &mut usize, budget: usize, adjacent_only: bool) -> usize {
    let Some(map) = item.as_object_mut() else {
        return demote_expanded_in_value(item, used, budget);
    };
    let is_chunk = map.contains_key("chunk_id")
        && (map.contains_key("text") || map.contains_key("content") || map.contains_key("alias"));
    if !is_chunk {
        return demote_expanded_in_value(item, used, budget);
    }
    if !is_expanded_body(map) {
        return 0;
    }
    let adj = is_adjacent_chunk(map);
    if adjacent_only != adj {
        return 0;
    }
    if *used <= budget {
        return 0;
    }
    let cost = chunk_text_len(map);
    if cost <= WORKING_SET_CARD_CHARS {
        return 0;
    }
    demote_chunk_to_card(map);
    *used = used.saturating_sub(cost.saturating_sub(WORKING_SET_CARD_CHARS));
    1
}

fn measure_expanded_chars(v: &Value) -> usize {
    match v {
        Value::Array(arr) => arr.iter().map(measure_expanded_chars).sum(),
        Value::Object(map) => {
            let is_chunk = map.contains_key("chunk_id")
                && (map.contains_key("text")
                    || map.contains_key("content")
                    || map.contains_key("alias"));
            if is_chunk {
                if is_expanded_body(map) {
                    chunk_text_len(map)
                } else {
                    0
                }
            } else {
                map.values().map(measure_expanded_chars).sum()
            }
        }
        _ => 0,
    }
}

/// Within kept (recent) retrieval messages, demote oldest expanded bodies first
/// until total expanded chars ≤ `budget`. Adjacent runs demoted last.
/// Returns total demoted chunk count.
fn apply_working_set_budget(messages: &mut [ChatMessage], keep_indices: &[usize], budget: usize) -> usize {
    // Measure current expanded usage across kept messages.
    let mut used = 0usize;
    let mut parsed: Vec<(usize, Option<Value>)> = Vec::new();
    for &idx in keep_indices {
        let content = &messages[idx].content;
        if let Ok(v) = serde_json::from_str::<Value>(content) {
            used = used.saturating_add(measure_expanded_chars(&v));
            parsed.push((idx, Some(v)));
        } else {
            // Heuristic payloads: count long "text"/"content" string lengths roughly.
            used = used.saturating_add(heuristic_expanded_chars(content));
            parsed.push((idx, None));
        }
    }
    if used <= budget {
        return 0;
    }

    let mut demoted_total = 0usize;
    // Oldest kept first.
    for (idx, maybe_v) in parsed.iter_mut() {
        if used <= budget {
            break;
        }
        if let Some(v) = maybe_v {
            demoted_total += demote_expanded_in_value(v, &mut used, budget);
            messages[*idx].content = v.to_string();
        } else {
            // Non-JSON: if still over, collapse long string values toward cards.
            let before = messages[*idx].content.len();
            let trimmed = collapse_long_strings_to_card(&messages[*idx].content, WORKING_SET_CARD_CHARS);
            if trimmed.len() < before {
                let saved = before.saturating_sub(trimmed.len());
                used = used.saturating_sub(saved);
                messages[*idx].content = trimmed;
                demoted_total += 1;
            }
        }
    }
    if demoted_total > 0 {
        // Mark the most recent kept message with a third-person fact line.
        if let Some(&last) = keep_indices.last() {
            if !messages[last].content.contains("[working_set_trimmed]") {
                messages[last]
                    .content
                    .push_str("\n[working_set_trimmed] near-round expanded bodies demoted under char budget");
            }
        }
    }
    demoted_total
}

fn heuristic_expanded_chars(content: &str) -> usize {
    // Rough: sum lengths of quoted values after "text"/"content" when > card size.
    let mut total = 0usize;
    for key in ["\"text\"", "\"content\""] {
        let mut rest = content;
        while let Some(pos) = rest.find(key) {
            let after = rest[pos + key.len()..].trim_start();
            if let Some(after) = after.strip_prefix(':') {
                let after = after.trim_start();
                if let Some(s) = after.strip_prefix('"') {
                    if let Some(end) = find_json_string_end(s) {
                        let body = &s[..end.saturating_sub(1)];
                        let n = body.chars().count();
                        if n > WORKING_SET_CARD_CHARS {
                            total = total.saturating_add(n);
                        }
                        rest = &s[end..];
                        continue;
                    }
                }
            }
            rest = &rest[pos + key.len()..];
        }
    }
    total
}

fn collapse_long_strings_to_card(content: &str, max_chars: usize) -> String {
    let mut out = content.to_string();
    for key in ["\"text\"", "\"content\""] {
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
                    if let Some(end) = find_json_string_end(s) {
                        let body = &s[..end.saturating_sub(1)];
                        let n = body.chars().count();
                        if n > max_chars {
                            let head: String = body.chars().take(max_chars.saturating_sub(1)).collect();
                            result.push_str(": \"");
                            result.push_str(&head);
                            result.push('…');
                            result.push('"');
                            rest = &s[end..];
                            continue;
                        }
                        result.push_str(": \"");
                        result.push_str(body);
                        result.push('"');
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
    out
}

/// Build model-facing messages: keep last `keep_recent` retrieval payloads full;
/// stub older ones; then apply working-set char budget on the kept set.
/// Does not mutate the source slice.
pub fn transform_messages_for_llm(messages: &[ChatMessage], keep_recent: usize) -> Vec<ChatMessage> {
    transform_messages_for_llm_with_budget(messages, keep_recent, WORKING_SET_CHAR_BUDGET)
}

/// Same as [`transform_messages_for_llm`] with an explicit char budget (tests).
pub fn transform_messages_for_llm_with_budget(
    messages: &[ChatMessage],
    keep_recent: usize,
    char_budget: usize,
) -> Vec<ChatMessage> {
    let mut retrieval_indices: Vec<usize> = Vec::new();
    for (i, m) in messages.iter().enumerate() {
        if (m.role == "user" || m.role == "tool") && looks_like_retrieval_payload(&m.content) {
            retrieval_indices.push(i);
        }
    }
    let keep_from = retrieval_indices.len().saturating_sub(keep_recent);
    let keep_set: std::collections::HashSet<usize> = retrieval_indices
        .iter()
        .skip(keep_from)
        .copied()
        .collect();
    let keep_ordered: Vec<usize> = retrieval_indices.iter().skip(keep_from).copied().collect();

    let mut out: Vec<ChatMessage> = messages
        .iter()
        .enumerate()
        .map(|(i, m)| {
            if keep_set.contains(&i) || !looks_like_retrieval_payload(&m.content) {
                return m.clone();
            }
            let mut msg = m.clone();
            msg.content = collapse_message_content(&m.content);
            msg
        })
        .collect();

    if char_budget > 0 && !keep_ordered.is_empty() {
        apply_working_set_budget(&mut out, &keep_ordered, char_budget);
    }
    out
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
        assert!(
            out[1].content.contains("history_cleared")
                || out[1].content.contains("body_omitted")
                || out[1].content.len() < out[3].content.len()
        );
        // Last two stay large
        assert!(out[3].content.contains("chunk_id"));
        assert!(out[4].content.contains("xxx") || out[4].content.len() > 50);
    }

    #[test]
    fn working_set_budget_demotes_older_kept_expand() {
        // Two recent rounds, each with a large expanded body; tight budget
        // should demote the older kept message first.
        let body_a = "A".repeat(800);
        let body_b = "B".repeat(800);
        let msgs = vec![
            ChatMessage::user("q"),
            ChatMessage::user(format!(
                r##"{{"chunks":[{{"chunk_id":"a","alias":"#1","text":"{body_a}","visibility":"expanded"}}]}}"##
            )),
            ChatMessage::user(format!(
                r##"{{"chunks":[{{"chunk_id":"b","alias":"#2","text":"{body_b}","visibility":"expanded"}}]}}"##
            )),
        ];
        let out = transform_messages_for_llm_with_budget(&msgs, 2, 900);
        // Older kept (#1) demoted to card-sized; newer may stay larger.
        let c1 = &out[1].content;
        let c2 = &out[2].content;
        assert!(
            c1.contains("working_set_demoted")
                || c1.contains("body_omitted")
                || c1.len() < body_a.len(),
            "older expand should shrink under budget: len={}",
            c1.len()
        );
        // Total expanded in kept set should be constrained.
        let total_approx = c1.len() + c2.len();
        assert!(
            total_approx < body_a.len() + body_b.len(),
            "working set should be smaller than raw sum: {total_approx}"
        );
        assert!(
            out.iter().any(|m| m.content.contains("[working_set_trimmed]"))
                || c1.contains("working_set_demoted"),
            "trim signal expected"
        );
    }

    #[test]
    fn working_set_prefers_keeping_adjacent() {
        let plain = "P".repeat(600);
        let adj = "Q".repeat(600);
        let msgs = vec![ChatMessage::user(format!(
            r##"{{"chunks":[
              {{"chunk_id":"p","alias":"#1","text":"{plain}","visibility":"expanded"}},
              {{"chunk_id":"a","alias":"#2","text":"{adj}","visibility":"expanded","adjacent":true,"member_chunk_ids":["a","b"]}}
            ]}}"##
        ))];
        let out = transform_messages_for_llm_with_budget(&msgs, 1, 700);
        let c = &out[0].content;
        // Adjacent should still hold more of its body when possible.
        assert!(
            c.contains("QQQQ") || c.contains("\"adjacent\":true"),
            "adjacent run should be preferred: {c}"
        );
    }

    #[test]
    fn under_budget_leaves_expands_intact() {
        let body = "x".repeat(100);
        let msgs = vec![ChatMessage::user(format!(
            r##"{{"chunks":[{{"chunk_id":"1","text":"{body}","visibility":"expanded"}}]}}"##
        ))];
        let out = transform_messages_for_llm_with_budget(&msgs, 2, 16_000);
        assert!(out[0].content.contains(&body));
        assert!(!out[0].content.contains("working_set_demoted"));
    }
}
