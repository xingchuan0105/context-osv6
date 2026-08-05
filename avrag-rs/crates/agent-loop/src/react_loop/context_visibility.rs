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
/// Soft cap for expanded chunk text retained across the working set
/// (Unicode scalar / char count, not UTF-8 bytes). Shared account for near-round
/// full bodies after history stub (U-P2 / D4).
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
        let mut out = v.to_string();
        out.push('\n');
        out.push_str(super::prompt_assets::history_cleared());
        return out;
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
        out.push('\n');
        out.push_str(super::prompt_assets::history_cleared());
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

/// Demote expanded chunks matching `want_adjacent` until `used` ≤ budget.
/// After demote, cards no longer count as expanded — subtract full pre-demote cost.
fn demote_matching_in_value(
    v: &mut Value,
    used: &mut usize,
    budget: usize,
    want_adjacent: bool,
) -> usize {
    let mut demoted = 0usize;
    match v {
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                if *used <= budget {
                    break;
                }
                demoted += demote_matching_in_value(item, used, budget, want_adjacent);
            }
        }
        Value::Object(map) => {
            let is_chunk = map.contains_key("chunk_id")
                && (map.contains_key("text")
                    || map.contains_key("content")
                    || map.contains_key("alias"));
            if is_chunk {
                if !is_expanded_body(map) {
                    return 0;
                }
                if is_adjacent_chunk(map) != want_adjacent {
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
                // Card no longer counts toward expanded; drop full pre-demote cost.
                *used = used.saturating_sub(cost);
                demoted = 1;
            } else {
                // Prefer demoting nested chunks under "chunks" / "hits" first.
                for key in ["chunks", "hits"] {
                    if *used <= budget {
                        break;
                    }
                    if let Some(child) = map.get_mut(key) {
                        demoted += demote_matching_in_value(child, used, budget, want_adjacent);
                    }
                }
                let keys: Vec<String> = map
                    .keys()
                    .filter(|k| *k != "chunks" && *k != "hits")
                    .cloned()
                    .collect();
                for k in keys {
                    if *used <= budget {
                        break;
                    }
                    if let Some(child) = map.get_mut(&k) {
                        demoted += demote_matching_in_value(child, used, budget, want_adjacent);
                    }
                }
            }
        }
        _ => {}
    }
    demoted
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

/// Within kept (recent) retrieval messages, demote expanded bodies until total
/// expanded chars ≤ `budget`.
///
/// Global order (U-P2 / §6.3):
/// 1. non-adjacent expands, oldest kept message → newest
/// 2. only then adjacent runs, oldest → newest
///
/// Returns total demoted chunk count.
fn apply_working_set_budget(
    messages: &mut [ChatMessage],
    keep_indices: &[usize],
    budget: usize,
) -> usize {
    // Measure current expanded usage across kept messages.
    let mut used = 0usize;
    let mut parsed: Vec<(usize, Option<Value>)> = Vec::new();
    for &idx in keep_indices {
        let content = &messages[idx].content;
        if let Ok(v) = serde_json::from_str::<Value>(content) {
            used = used.saturating_add(measure_expanded_chars(&v));
            parsed.push((idx, Some(v)));
        } else {
            used = used.saturating_add(heuristic_expanded_chars(content));
            parsed.push((idx, None));
        }
    }
    if used <= budget {
        return 0;
    }

    let mut demoted_total = 0usize;

    // Pass 1: non-adjacent (global, oldest first). Pass 2: adjacent last.
    for want_adjacent in [false, true] {
        if used <= budget {
            break;
        }
        for (idx, maybe_v) in parsed.iter_mut() {
            if used <= budget {
                break;
            }
            if let Some(v) = maybe_v {
                let n = demote_matching_in_value(v, &mut used, budget, want_adjacent);
                if n > 0 {
                    messages[*idx].content = v.to_string();
                    demoted_total += n;
                }
            } else if !want_adjacent {
                // Heuristic payloads have no adjacent flag — demote in pass 1 only.
                let before_chars = heuristic_expanded_chars(&messages[*idx].content);
                if before_chars == 0 {
                    continue;
                }
                let trimmed =
                    collapse_long_strings_to_card(&messages[*idx].content, WORKING_SET_CARD_CHARS);
                let after_chars = heuristic_expanded_chars(&trimmed);
                if after_chars < before_chars {
                    used = used.saturating_sub(before_chars.saturating_sub(after_chars));
                    messages[*idx].content = trimmed;
                    demoted_total += 1;
                }
            }
        }
    }

    if demoted_total > 0 {
        if let Some(&last) = keep_indices.last() {
            if !messages[last].content.contains("[working_set_trimmed]") {
                messages[last].content.push('\n');
                messages[last]
                    .content
                    .push_str(super::prompt_assets::working_set_trimmed());
            }
        }
    }
    demoted_total
}

/// Sum long string values after `"text"` (preferred) or, if none, `"content"`.
/// Matches JSON measure precedence: text OR content, not both.
fn heuristic_expanded_chars(content: &str) -> usize {
    let text_total = sum_long_quoted_after_key(content, "\"text\"");
    if text_total > 0 {
        return text_total;
    }
    sum_long_quoted_after_key(content, "\"content\"")
}

fn sum_long_quoted_after_key(content: &str, key: &str) -> usize {
    let mut total = 0usize;
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
    total
}

fn collapse_long_strings_to_card(content: &str, max_chars: usize) -> String {
    // Prefer demoting text fields; if none changed, demote content.
    let via_text = collapse_key_long_strings(content, "\"text\"", max_chars);
    if via_text != content {
        return via_text;
    }
    collapse_key_long_strings(content, "\"content\"", max_chars)
}

fn collapse_key_long_strings(content: &str, key: &str, max_chars: usize) -> String {
    let mut result = String::with_capacity(content.len());
    let mut rest = content;
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
    result
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
    use serde_json::Value;

    fn expanded_chars_in_messages(msgs: &[ChatMessage], indices: &[usize]) -> usize {
        let mut total = 0usize;
        for &i in indices {
            if let Ok(v) = serde_json::from_str::<Value>(&msgs[i].content) {
                total = total.saturating_add(measure_expanded_chars(&v));
            }
        }
        total
    }

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
        // Durable input unchanged.
        assert!(msgs[1].content.contains("xxx") || msgs[1].content.contains("x".repeat(50).as_str()));
    }

    #[test]
    fn working_set_budget_demotes_older_kept_expand() {
        // Two recent rounds, each with a large expanded body; budget fits one full
        // body (800) — older should demote, newer stay largely intact.
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
        let budget = 900usize;
        let out = transform_messages_for_llm_with_budget(&msgs, 2, budget);
        let c1 = &out[1].content;
        let c2 = &out[2].content;

        assert!(
            c1.contains("working_set_demoted") || c1.contains("body_omitted"),
            "older expand should demote: {c1}"
        );
        assert!(
            c2.contains(&body_b) || c2.matches('B').count() > 500,
            "newer expand should remain largely intact: len={}",
            c2.len()
        );
        let remaining = expanded_chars_in_messages(&out, &[1, 2]);
        assert!(
            remaining <= budget,
            "remaining expanded {remaining} should be ≤ budget {budget}"
        );
        assert!(
            out.iter().any(|m| m.content.contains("[working_set_trimmed]")),
            "trim signal expected"
        );
        // Source slice immutable.
        assert!(msgs[1].content.contains(&body_a));
        assert!(msgs[2].content.contains(&body_b));
    }

    #[test]
    fn working_set_prefers_keeping_adjacent_same_message() {
        let plain = "P".repeat(600);
        let adj = "Q".repeat(600);
        let msgs = vec![ChatMessage::user(format!(
            r##"{{"chunks":[
              {{"chunk_id":"p","alias":"#1","text":"{plain}","visibility":"expanded"}},
              {{"chunk_id":"a","alias":"#2","text":"{adj}","visibility":"expanded","adjacent":true,"member_chunk_ids":["a","b"]}}
            ]}}"##
        ))];
        let budget = 700usize;
        let out = transform_messages_for_llm_with_budget(&msgs, 1, budget);
        let c = &out[0].content;
        // Plain demoted first; adjacent body retained.
        assert!(
            c.contains("working_set_demoted") || c.contains("body_omitted"),
            "plain should demote: {c}"
        );
        assert!(
            c.contains(&adj) || c.matches('Q').count() > 500,
            "adjacent full body should be preferred: {c}"
        );
        let remaining = expanded_chars_in_messages(&out, &[0]);
        assert!(remaining <= budget, "remaining {remaining} > budget {budget}");
    }

    #[test]
    fn working_set_demotes_newer_plain_before_older_adjacent() {
        // Cross-message: older = adjacent mega, newer = plain. Budget fits one body.
        // Global policy: demote plain first even though it is newer.
        let adj_body = "Q".repeat(600);
        let plain_body = "P".repeat(600);
        let msgs = vec![
            ChatMessage::user(format!(
                r##"{{"chunks":[{{"chunk_id":"a","alias":"#1","text":"{adj_body}","visibility":"expanded","adjacent":true,"member_chunk_ids":["a","b"]}}]}}"##
            )),
            ChatMessage::user(format!(
                r##"{{"chunks":[{{"chunk_id":"p","alias":"#2","text":"{plain_body}","visibility":"expanded"}}]}}"##
            )),
        ];
        let budget = 700usize;
        let out = transform_messages_for_llm_with_budget(&msgs, 2, budget);
        let older = &out[0].content;
        let newer = &out[1].content;
        assert!(
            older.contains(&adj_body) || older.matches('Q').count() > 500,
            "older adjacent should survive: {older}"
        );
        assert!(
            newer.contains("working_set_demoted") || newer.contains("body_omitted"),
            "newer plain should demote first: {newer}"
        );
        let remaining = expanded_chars_in_messages(&out, &[0, 1]);
        assert!(remaining <= budget, "remaining {remaining} > budget {budget}");
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
        assert!(!out[0].content.contains("[working_set_trimmed]"));
    }

    #[test]
    fn does_not_over_demote_when_one_suffices() {
        // 3×500 expands, budget 1100 → demote exactly two oldest, keep newest full.
        let bodies: Vec<String> = (0..3).map(|i| format!("{}", (b'A' + i as u8) as char).repeat(500)).collect();
        let msgs: Vec<ChatMessage> = bodies
            .iter()
            .enumerate()
            .map(|(i, b)| {
                ChatMessage::user(format!(
                    r##"{{"chunks":[{{"chunk_id":"{i}","alias":"#{i}","text":"{b}","visibility":"expanded"}}]}}"##
                ))
            })
            .collect();
        let budget = 1100usize;
        let out = transform_messages_for_llm_with_budget(&msgs, 3, budget);
        let remaining = expanded_chars_in_messages(&out, &[0, 1, 2]);
        assert!(remaining <= budget, "remaining {remaining} > {budget}");
        // Newest should still hold ~500 expanded chars.
        assert!(
            out[2].content.contains(&bodies[2]),
            "newest expand should remain when demoting two older is enough"
        );
        // Should not demote all three: remaining should be close to 500 (one full).
        assert!(
            remaining >= 400,
            "should not over-demote; remaining expanded={remaining}"
        );
    }
}
