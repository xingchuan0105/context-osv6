//! Model-visible evidence plan for bridge chunk lists (S+L × P1+ unification).
//!
//! After alias assignment, apply expand / card / stub to each chunk object.
//! Durable full text is recorded in `body_store` for reseen closure.

use serde_json::{json, Value};

/// Max chars kept as snippet for card form.
pub const CARD_SNIPPET_CHARS: usize = 280;
/// Soft budget for expanded full bodies per bridge call (UTF-8 chars).
pub const EXPAND_CHAR_BUDGET_PER_CALL: usize = 12_000;

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

pub fn is_adjacent_item(item: &Value) -> bool {
    item.get("adjacent").and_then(|v| v.as_bool()) == Some(true)
        || item
            .get("source")
            .and_then(|v| v.as_str())
            .is_some_and(|s| s.contains("+adjacent"))
        || member_ids_from_item(item).len() > 1
}

fn text_field<'a>(item: &'a Value) -> Option<&'a str> {
    item.get("text")
        .and_then(|v| v.as_str())
        .or_else(|| item.get("content").and_then(|v| v.as_str()))
}

fn set_text_fields(item: &mut Value, text: &str) {
    if let Some(obj) = item.as_object_mut() {
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
}

fn snippet(s: &str, max_chars: usize) -> String {
    let n = s.chars().count();
    if n <= max_chars {
        return s.to_string();
    }
    let head: String = s.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{head}…")
}

fn mark_expanded(item: &mut Value, full: &str) {
    set_text_fields(item, full);
    if let Some(obj) = item.as_object_mut() {
        obj.insert("visibility".into(), json!("expanded"));
        obj.remove("body_omitted");
        obj.remove("snippet_truncated");
    }
}

fn mark_card(item: &mut Value, full: &str) {
    let snip = snippet(full, CARD_SNIPPET_CHARS);
    set_text_fields(item, &snip);
    if let Some(obj) = item.as_object_mut() {
        obj.insert("visibility".into(), json!("card"));
        obj.insert("body_omitted".into(), json!(true));
        if full.chars().count() > CARD_SNIPPET_CHARS {
            obj.insert("snippet_truncated".into(), json!(true));
        }
    }
}

fn mark_stub(item: &mut Value) {
    set_text_fields(item, "");
    if let Some(obj) = item.as_object_mut() {
        obj.insert("visibility".into(), json!("stub"));
        obj.insert("body_omitted".into(), json!(true));
    }
}

/// Apply visibility to a mutable chunks array after alias/reseen assignment.
///
/// Policy (unification design):
/// 1. Reseen → stub
/// 2. Adjacent runs → always expand (q017)
/// 3. Other new hits → expand until `expand_budget` chars, then card
///
/// `body_store`: durable full text keyed by any member chunk_id.
/// Returns `(expanded_n, card_n, stub_n, expand_chars_used)`.
pub fn apply_visibility_to_chunks(
    items: &mut [Value],
    body_store: &mut std::collections::HashMap<String, String>,
    expand_budget: usize,
) -> (usize, usize, usize, usize) {
    // Capture durable bodies for any non-empty text on new items.
    for item in items.iter() {
        if item.get("reseen").is_some() {
            continue;
        }
        if let Some(full) = text_field(item).filter(|t| !t.is_empty()) {
            for mid in member_ids_from_item(item) {
                body_store.entry(mid).or_insert_with(|| full.to_string());
            }
        }
    }

    let mut used = 0usize;
    let mut expanded = 0usize;
    let mut cards = 0usize;
    let mut stubs = 0usize;

    // Pass A: stubs + forced adjacent expands
    for item in items.iter_mut() {
        if item.get("reseen").is_some() {
            mark_stub(item);
            stubs += 1;
            continue;
        }
        if is_adjacent_item(item) {
            let full = text_field(item)
                .filter(|t| !t.is_empty())
                .map(str::to_string)
                .or_else(|| {
                    member_ids_from_item(item)
                        .into_iter()
                        .find_map(|id| body_store.get(&id).cloned())
                })
                .unwrap_or_default();
            mark_expanded(item, &full);
            used = used.saturating_add(full.chars().count());
            expanded += 1;
        }
    }

    // Pass B: non-adjacent new hits — expand under remaining budget, else card
    for item in items.iter_mut() {
        if item.get("reseen").is_some() {
            continue;
        }
        if is_adjacent_item(item) {
            continue;
        }
        let full = text_field(item)
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .or_else(|| {
                member_ids_from_item(item)
                    .into_iter()
                    .find_map(|id| body_store.get(&id).cloned())
            })
            .unwrap_or_default();
        let cost = full.chars().count();
        if cost > 0 && used.saturating_add(cost) <= expand_budget {
            mark_expanded(item, &full);
            used = used.saturating_add(cost);
            expanded += 1;
        } else {
            mark_card(item, &full);
            cards += 1;
        }
    }

    (expanded, cards, stubs, used)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn adjacent_expands_even_when_large() {
        let mut items = vec![json!({
            "chunk_id": "a",
            "member_chunk_ids": ["a", "b"],
            "adjacent": true,
            "text": "x".repeat(500),
            "alias": "#1",
        })];
        let mut store = std::collections::HashMap::new();
        let (e, c, s, _) = apply_visibility_to_chunks(&mut items, &mut store, 100);
        assert_eq!((e, c, s), (1, 0, 0));
        assert_eq!(items[0]["text"].as_str().unwrap().len(), 500);
        assert_eq!(items[0]["visibility"], "expanded");
    }

    #[test]
    fn plain_hit_cards_when_budget_tight() {
        let mut items = vec![
            json!({"chunk_id": "1", "text": "a".repeat(400), "alias": "#1"}),
            json!({"chunk_id": "2", "text": "b".repeat(400), "alias": "#2"}),
        ];
        let mut store = std::collections::HashMap::new();
        let (e, c, s, _) = apply_visibility_to_chunks(&mut items, &mut store, 450);
        assert_eq!(s, 0);
        assert_eq!(e + c, 2);
        assert!(e >= 1);
        assert!(c >= 1);
    }

    #[test]
    fn reseen_is_stub() {
        let mut items = vec![json!({
            "chunk_id": "1",
            "alias": "#1",
            "reseen": "#1",
            "text": "",
        })];
        let mut store = std::collections::HashMap::new();
        let (e, c, s, _) = apply_visibility_to_chunks(&mut items, &mut store, 12_000);
        assert_eq!((e, c, s), (0, 0, 1));
        assert_eq!(items[0]["visibility"], "stub");
    }
}
