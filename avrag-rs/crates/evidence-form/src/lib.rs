//! Evidence Form — pure model-visible morphology for retrieval hit JSON objects.
//!
//! Owns: expanded / card / stub marks, adjacent predicate, card snippet length,
//! `text`/`content` field conventions, char measurement helpers.
//!
//! Does **not** own: per-call expand budget, working-set budget, history keep-K,
//! alias/reseen policy, Evidence Pool lifecycle, or prompt copy.

use serde_json::{json, Map, Value};

/// Max Unicode scalar chars kept as snippet for card form (single source).
pub const CARD_SNIPPET_CHARS: usize = 280;

/// Collect member chunk ids from a chunk JSON item.
pub fn member_ids_from_item(item: &Value) -> Vec<String> {
    if let Some(arr) = item.get("member_chunk_ids").and_then(|v| v.as_array()) {
        let ids: Vec<String> = arr
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        if !ids.is_empty() {
            return ids;
        }
    }
    item.get("chunk_id")
        .and_then(|v| v.as_str())
        .map(|s| vec![s.to_string()])
        .unwrap_or_default()
}

/// True when the item is an S+L adjacent / multi-member run.
pub fn is_adjacent_item(item: &Value) -> bool {
    item.get("adjacent").and_then(|v| v.as_bool()) == Some(true)
        || item
            .get("source")
            .and_then(|v| v.as_str())
            .is_some_and(|s| s.contains("+adjacent"))
        || member_ids_from_item(item).len() > 1
}

/// Object-map variant of [`is_adjacent_item`].
pub fn is_adjacent_map(map: &Map<String, Value>) -> bool {
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

/// Prefer `text`, else `content`.
pub fn text_field<'a>(item: &'a Value) -> Option<&'a str> {
    item.get("text")
        .and_then(|v| v.as_str())
        .or_else(|| item.get("content").and_then(|v| v.as_str()))
}

/// Prefer `text`, else `content` (map).
pub fn text_field_map<'a>(map: &'a Map<String, Value>) -> Option<&'a str> {
    map.get("text")
        .and_then(|v| v.as_str())
        .or_else(|| map.get("content").and_then(|v| v.as_str()))
}

/// Write text into whichever of `text` / `content` already exist; else set `text`.
pub fn set_text_fields(item: &mut Value, text: &str) {
    if let Some(obj) = item.as_object_mut() {
        set_text_fields_map(obj, text);
    }
}

pub fn set_text_fields_map(obj: &mut Map<String, Value>, text: &str) {
    let has_text = obj.contains_key("text");
    let has_content = obj.contains_key("content");
    if has_text {
        obj.insert("text".into(), json!(text));
    }
    if has_content {
        obj.insert("content".into(), json!(text));
    }
    if !has_text && !has_content {
        obj.insert("text".into(), json!(text));
    }
}

/// Truncate to `max_chars` with a trailing ellipsis when needed.
pub fn snippet(s: &str, max_chars: usize) -> String {
    let n = s.chars().count();
    if n <= max_chars {
        return s.to_string();
    }
    let head: String = s.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{head}…")
}

/// Card snippet using [`CARD_SNIPPET_CHARS`].
pub fn card_snippet(s: &str) -> String {
    snippet(s, CARD_SNIPPET_CHARS)
}

pub fn mark_expanded(item: &mut Value, full: &str) {
    if let Some(obj) = item.as_object_mut() {
        mark_expanded_map(obj, full);
    }
}

fn mark_expanded_map(obj: &mut Map<String, Value>, full: &str) {
    set_text_fields_map(obj, full);
    obj.insert("visibility".into(), json!("expanded"));
    obj.remove("body_omitted");
    obj.remove("snippet_truncated");
}

pub fn mark_card(item: &mut Value, full: &str) {
    if let Some(obj) = item.as_object_mut() {
        mark_card_map(obj, full);
    }
}

/// Demote to card form in-place on an object map (full body already known).
pub fn mark_card_map(obj: &mut Map<String, Value>, full: &str) {
    let snip = card_snippet(full);
    set_text_fields_map(obj, &snip);
    obj.insert("visibility".into(), json!("card"));
    obj.insert("body_omitted".into(), json!(true));
    if full.chars().count() > CARD_SNIPPET_CHARS {
        obj.insert("snippet_truncated".into(), json!(true));
    }
}

/// Demote expanded map to card using its current text fields as full body.
/// Matches LLM-boundary working-set demote (no `snippet_truncated` flag).
pub fn demote_map_to_card(obj: &mut Map<String, Value>) {
    let full = text_field_map(obj).unwrap_or("").to_string();
    let snip = card_snippet(&full);
    set_text_fields_map(obj, &snip);
    obj.insert("visibility".into(), json!("card"));
    obj.insert("body_omitted".into(), json!(true));
}

pub fn mark_stub(item: &mut Value) {
    set_text_fields(item, "");
    if let Some(obj) = item.as_object_mut() {
        mark_stub_map(obj);
    }
}

pub fn mark_stub_map(obj: &mut Map<String, Value>) {
    set_text_fields_map(obj, "");
    obj.insert("visibility".into(), json!("stub"));
    obj.insert("body_omitted".into(), json!(true));
}

/// Heuristic: object looks like a retrieval chunk item.
pub fn is_chunk_map(map: &Map<String, Value>) -> bool {
    map.contains_key("chunk_id")
        && (map.contains_key("text")
            || map.contains_key("content")
            || map.contains_key("alias"))
}

pub fn chunk_text_len_map(map: &Map<String, Value>) -> usize {
    text_field_map(map).map(|s| s.chars().count()).unwrap_or(0)
}

/// Expanded = has body beyond card threshold and not stub/card/reseen-omitted.
pub fn is_expanded_body_map(map: &Map<String, Value>) -> bool {
    if map.get("body_omitted").and_then(|v| v.as_bool()) == Some(true) {
        return false;
    }
    if map.get("reseen").is_some() {
        return false;
    }
    match map.get("visibility").and_then(|v| v.as_str()) {
        Some("stub") | Some("card") => return false,
        _ => {}
    }
    chunk_text_len_map(map) > CARD_SNIPPET_CHARS
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn adjacent_from_members() {
        let v = json!({
            "chunk_id": "a",
            "member_chunk_ids": ["a", "b"],
            "text": "x"
        });
        assert!(is_adjacent_item(&v));
    }

    #[test]
    fn card_truncates() {
        let mut v = json!({"chunk_id": "1", "text": "x".repeat(400)});
        mark_card(&mut v, &"x".repeat(400));
        assert_eq!(v["visibility"], "card");
        assert!(v["body_omitted"].as_bool().unwrap());
        assert!(v["text"].as_str().unwrap().chars().count() <= CARD_SNIPPET_CHARS);
    }

    #[test]
    fn expanded_clears_body_omitted() {
        let mut v = json!({
            "chunk_id": "1",
            "text": "",
            "body_omitted": true,
            "visibility": "stub"
        });
        mark_expanded(&mut v, "hello");
        assert_eq!(v["text"], "hello");
        assert_eq!(v["visibility"], "expanded");
        assert!(v.get("body_omitted").is_none());
    }
}
