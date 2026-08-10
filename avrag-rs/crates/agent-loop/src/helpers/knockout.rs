//! Evidence knockout (SaC line protocol + host state).
//!
//! Design: `docs/engineering/2026-08-07-evidence-knockout-design.md` (V1 hard
//! suppress) + W4 policy in the KB/EWS design: **hard visibility suppress is off**.
//!
//! `KNOCKOUT:` lines may still parse/register for observability, but the host
//! **does not** strip bridge / tool payloads. Noise path = KEEP + host fold.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex};

use contracts::{ToolResult, ToolStatus};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Soft cap on ids listed in a single reexposure observation.
const REEXPOSE_LIST_MAX: usize = 12;
/// Cap registered / reexposed id lists in observability snapshots.
const OBS_ID_LIST_MAX: usize = 32;

/// Product policy W4 (2026-08-07): hard filter **off**.
/// When false, `apply_to_bridge_data` / `align_*` never strip chunks.
pub const KNOCKOUT_HARD_SUPPRESS: bool = false;

/// Shared run-scoped ledger (Arc so bridge + loop both hold it).
pub type SharedKnockout = Arc<Mutex<KnockoutState>>;

pub fn shared_knockout() -> SharedKnockout {
    Arc::new(Mutex::new(KnockoutState::new()))
}

/// White-box snapshot for `mode_debug.general.knockout` / eval artifacts.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct KnockoutObservability {
    /// Product hard-filter flag (W4: always false in product builds).
    pub hard_suppress_enabled: bool,
    /// Distinct ids successfully registered from `KNOCKOUT:` this run.
    pub registered_count: usize,
    /// Cap-truncated list of registered ids (lowercase UUID).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub registered_ids: Vec<String>,
    /// Distinct chunk_ids seen in Ok retrieval payloads.
    pub seen_count: usize,
    /// Times a knocked chunk was stripped from bridge JSON (hits 1–2).
    pub suppress_events: u32,
    /// Times a knocked chunk was re-exposed (post_knock_hits ≥ 3).
    pub reexpose_events: u32,
    /// Distinct ids that were re-exposed at least once.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reexposed_ids: Vec<String>,
    /// Current post-knock hit counters for registered ids only.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub post_knock_hits: BTreeMap<String, u8>,
}

/// Snapshot shared ledger (lock poison → empty default).
pub fn knockout_observability(ko: &SharedKnockout) -> KnockoutObservability {
    ko.lock()
        .map(|g| g.observability_snapshot())
        .unwrap_or_default()
}

/// Run-scoped knockout ledger (one agent turn / full149 question).
#[derive(Debug, Clone, Default)]
pub struct KnockoutState {
    /// Chunk ids the model has named as noise (normalized lowercase UUID).
    knocked: HashSet<String>,
    /// Hits **after** registration (design: post_knock_hits).
    post_knock_hits: HashMap<String, u8>,
    /// Chunk ids observed in any Ok retrieval payload this run.
    seen: HashSet<String>,
    /// Reexposed ids since last `take_reexposed` (for loop observation).
    pending_reexposed: Vec<String>,
    /// Cumulative suppress events (chunk dropped from bridge payload).
    suppress_events: u32,
    /// Cumulative reexpose events (chunk kept after ≥3 post-knock hits).
    reexpose_events: u32,
    /// Distinct ids re-exposed at least once this run.
    reexposed_all: HashSet<String>,
}

impl KnockoutState {
    pub fn new() -> Self {
        Self::default()
    }

    fn norm_id(id: &str) -> String {
        id.trim().to_ascii_lowercase()
    }

    /// Record chunk_ids that appeared in Ok retrieval (any tool).
    pub fn note_seen_from_tool_results(&mut self, tool_results: &[ToolResult]) {
        for tr in tool_results {
            if tr.status != ToolStatus::Ok {
                continue;
            }
            for id in chunk_ids_in_tool_data(tr.data.as_ref()) {
                self.seen.insert(Self::norm_id(&id));
            }
        }
    }

    /// Register knockout ids from model text. Only **seen** + valid UUID ids apply.
    /// Returns how many new ids were registered this call.
    pub fn register_from_model_text(&mut self, text: &str) -> usize {
        let parsed = parse_knockout_chunk_ids(text);
        let mut n = 0usize;
        for id in parsed {
            let id = Self::norm_id(&id);
            if !self.seen.contains(&id) {
                continue;
            }
            if self.knocked.insert(id.clone()) {
                self.post_knock_hits.entry(id).or_insert(0);
                n += 1;
            }
        }
        n
    }

    pub fn is_knocked(&self, chunk_id: &str) -> bool {
        self.knocked.contains(&Self::norm_id(chunk_id))
    }

    /// Apply knockout to bridge JSON **before** it is returned to Python.
    ///
    /// When [`KNOCKOUT_HARD_SUPPRESS`] is false (product default W4), this is a
    /// **no-op**: payloads are never stripped and post-knock counters do not
    /// advance. Legacy hard-filter body remains for unit tests / revival.
    pub fn apply_to_bridge_data(&mut self, data: &mut Value) -> Vec<String> {
        if !KNOCKOUT_HARD_SUPPRESS {
            let _ = data;
            return Vec::new();
        }
        self.apply_to_bridge_data_hard(data)
    }

    /// Hard-filter implementation (only when `KNOCKOUT_HARD_SUPPRESS` is true).
    fn apply_to_bridge_data_hard(&mut self, data: &mut Value) -> Vec<String> {
        let mut reexposed = Vec::new();
        if let Some(items) = data.get_mut("chunks").and_then(|v| v.as_array_mut()) {
            self.filter_chunk_array(items, &mut reexposed);
        } else if let Some(items) = data.as_array_mut() {
            self.filter_chunk_array(items, &mut reexposed);
        }
        if let Some(hits) = data.get_mut("hits").and_then(|v| v.as_array_mut()) {
            self.filter_chunk_array(hits, &mut reexposed);
        }
        for id in &reexposed {
            if !self.pending_reexposed.iter().any(|x| x == id) {
                self.pending_reexposed.push(id.clone());
            }
        }
        reexposed
    }

    /// Drain reexposed ids accumulated since last take (for loop observation).
    pub fn take_reexposed(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_reexposed)
    }

    /// Filter tool_results in place **without** re-counting (bridge already counted).
    /// Use when aligning captured ToolResult with already-applied bridge filter.
    pub fn align_tool_results_no_count(&self, tool_results: &mut [ToolResult]) {
        for tr in tool_results.iter_mut() {
            if tr.status != ToolStatus::Ok {
                continue;
            }
            if let Some(data) = tr.data.as_mut() {
                self.align_value_no_count(data);
            }
        }
    }

    /// Strip suppressed chunks from a JSON value (tool/bridge payload) without
    /// incrementing post-knock counters. No-op when hard suppress is off.
    pub fn align_value_no_count(&self, data: &mut Value) {
        if !KNOCKOUT_HARD_SUPPRESS {
            let _ = data;
            return;
        }
        if let Some(items) = data.get_mut("chunks").and_then(|v| v.as_array_mut()) {
            self.strip_suppressed_only(items);
        } else if let Some(items) = data.as_array_mut() {
            self.strip_suppressed_only(items);
        }
        if let Some(hits) = data.get_mut("hits").and_then(|v| v.as_array_mut()) {
            self.strip_suppressed_only(hits);
        }
    }

    fn strip_suppressed_only(&self, items: &mut Vec<Value>) {
        items.retain(|item| {
            let Some(id) = item.get("chunk_id").and_then(|v| v.as_str()) else {
                return true;
            };
            let id = Self::norm_id(id);
            if !self.knocked.contains(&id) {
                return true;
            }
            let hits = self.post_knock_hits.get(&id).copied().unwrap_or(0);
            hits >= 3
        });
    }

    fn filter_chunk_array(&mut self, items: &mut Vec<Value>, reexposed: &mut Vec<String>) {
        let mut keep = Vec::with_capacity(items.len());
        for item in items.drain(..) {
            let id = item
                .get("chunk_id")
                .and_then(|v| v.as_str())
                .map(Self::norm_id);
            let Some(id) = id else {
                keep.push(item);
                continue;
            };
            self.seen.insert(id.clone());
            if !self.knocked.contains(&id) {
                keep.push(item);
                continue;
            }
            let hits = self.post_knock_hits.entry(id.clone()).or_insert(0);
            *hits = hits.saturating_add(1);
            if *hits >= 3 {
                self.reexpose_events = self.reexpose_events.saturating_add(1);
                self.reexposed_all.insert(id.clone());
                if !reexposed.contains(&id) {
                    reexposed.push(id);
                }
                keep.push(item);
            } else {
                self.suppress_events = self.suppress_events.saturating_add(1);
                // drop (suppress) — not returned to Python / not in observation
            }
        }
        *items = keep;
    }

    /// Format reexposed ids for loop observation (truncated).
    pub fn format_reexpose_list(ids: &[String]) -> String {
        let shown: Vec<&str> = ids.iter().take(REEXPOSE_LIST_MAX).map(String::as_str).collect();
        let mut s = shown.join(", ");
        if ids.len() > REEXPOSE_LIST_MAX {
            s.push_str(&format!(" …(+{})", ids.len() - REEXPOSE_LIST_MAX));
        }
        s
    }

    /// End-of-run white-box snapshot (stable field set for eval / mode_debug).
    pub fn observability_snapshot(&self) -> KnockoutObservability {
        let mut registered_ids: Vec<String> = self.knocked.iter().cloned().collect();
        registered_ids.sort();
        let registered_count = registered_ids.len();
        registered_ids.truncate(OBS_ID_LIST_MAX);

        let mut reexposed_ids: Vec<String> = self.reexposed_all.iter().cloned().collect();
        reexposed_ids.sort();
        reexposed_ids.truncate(OBS_ID_LIST_MAX);

        let mut post_knock_hits = BTreeMap::new();
        for id in &self.knocked {
            if let Some(&h) = self.post_knock_hits.get(id) {
                post_knock_hits.insert(id.clone(), h);
            }
        }

        KnockoutObservability {
            hard_suppress_enabled: KNOCKOUT_HARD_SUPPRESS,
            registered_count,
            registered_ids,
            seen_count: self.seen.len(),
            suppress_events: self.suppress_events,
            reexpose_events: self.reexpose_events,
            reexposed_ids,
            post_knock_hits,
        }
    }
}

/// Parse `KNOCKOUT:` / `敲除:` lines → chunk_id strings (valid UUID, dedupe, order).
pub fn parse_knockout_chunk_ids(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let line = trimmed
            .trim_start_matches(|c: char| c == '>' || c == '-' || c == '*' || c == '`' || c == '|')
            .trim_start();
        let line = line.trim_end_matches('`').trim_end();
        let Some(rest) = knockout_line_body(line) else {
            continue;
        };
        for token in rest.split(|c: char| c == ',' || c == '、' || c.is_whitespace()) {
            let token = token
                .trim()
                .trim_matches(|c: char| c == '`' || c == '"' || c == '\'');
            if token.is_empty() {
                continue;
            }
            if Uuid::parse_str(token).is_err() {
                continue;
            }
            let id = token.to_ascii_lowercase();
            if !out.iter().any(|x| x == &id) {
                out.push(id);
            }
        }
    }
    out
}

fn knockout_line_body(line: &str) -> Option<&str> {
    for prefix in ["KNOCKOUT", "Knockout", "knockout", "敲除"] {
        if let Some(rest) = line.strip_prefix(prefix) {
            let rest = rest.trim_start();
            let rest = rest.strip_prefix(':').or_else(|| rest.strip_prefix('：'))?;
            return Some(rest.trim());
        }
    }
    None
}

fn chunk_ids_in_tool_data(data: Option<&Value>) -> Vec<String> {
    let Some(data) = data else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let lists = [
        data.as_array(),
        data.get("chunks").and_then(|v| v.as_array()),
        data.get("hits").and_then(|v| v.as_array()),
    ];
    for list in lists.into_iter().flatten() {
        for item in list {
            if let Some(id) = item.get("chunk_id").and_then(|v| v.as_str()) {
                if !id.is_empty() {
                    out.push(id.to_ascii_lowercase());
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const ID_A: &str = "11111111-1111-1111-1111-111111111111";
    const ID_B: &str = "22222222-2222-2222-2222-222222222222";
    const ID_C: &str = "33333333-3333-3333-3333-333333333333";

    fn ok_chunks(ids: &[&str]) -> ToolResult {
        let chunks: Vec<Value> = ids
            .iter()
            .map(|id| json!({"chunk_id": id, "content": format!("body-{id}")}))
            .collect();
        ToolResult {
            tool: "dense_retrieval".into(),
            version: "1".into(),
            status: ToolStatus::Ok,
            data: Some(json!({"chunks": chunks})),
            trace: None,
        }
    }

    #[test]
    fn parse_knockout_line_uuid_and_chinese_prefix() {
        let t = format!("noise\nKNOCKOUT: {ID_A}, {ID_B}\nok");
        assert_eq!(
            parse_knockout_chunk_ids(&t),
            vec![ID_A.to_string(), ID_B.to_string()]
        );
        let t2 = format!("> 敲除： `{ID_C}`");
        assert_eq!(parse_knockout_chunk_ids(&t2), vec![ID_C.to_string()]);
        assert!(parse_knockout_chunk_ids("KNOCKOUT: #1, not-uuid").is_empty());
    }

    #[test]
    fn hard_suppress_off_never_strips_bridge_payload() {
        assert!(!KNOCKOUT_HARD_SUPPRESS);
        let mut ko = KnockoutState::new();
        let batch = vec![ok_chunks(&[ID_A, ID_B])];
        ko.note_seen_from_tool_results(&batch);
        assert_eq!(ko.register_from_model_text(&format!("KNOCKOUT: {ID_A}")), 1);
        assert!(ko.is_knocked(ID_A));

        let mut data = json!({"chunks": [
            {"chunk_id": ID_A, "content": "noise"},
            {"chunk_id": ID_B, "content": "keep"},
        ]});
        let re = ko.apply_to_bridge_data(&mut data);
        assert!(re.is_empty());
        // Both chunks remain — hard filter does not hide visibility.
        assert_eq!(data["chunks"].as_array().unwrap().len(), 2);
        assert_eq!(ko.observability_snapshot().suppress_events, 0);
        assert_eq!(ko.observability_snapshot().reexpose_events, 0);
        assert!(!ko.observability_snapshot().hard_suppress_enabled);
    }

    #[test]
    fn unseen_id_not_registered() {
        let mut ko = KnockoutState::new();
        assert_eq!(ko.register_from_model_text(&format!("KNOCKOUT: {ID_A}")), 0);
        assert!(!ko.is_knocked(ID_A));
    }

    #[test]
    fn uuid_case_normalized_register_only() {
        let mut ko = KnockoutState::new();
        let upper = ID_A.to_ascii_uppercase();
        let batch = vec![ok_chunks(&[ID_A])];
        ko.note_seen_from_tool_results(&batch);
        ko.register_from_model_text(&format!("KNOCKOUT: {upper}"));
        assert!(ko.is_knocked(ID_A));
        let mut data = json!({"chunks": [{"chunk_id": ID_A, "content": "x"}]});
        ko.apply_to_bridge_data(&mut data);
        // Hard suppress off: still present after apply.
        assert_eq!(data["chunks"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn align_value_no_count_is_noop_when_hard_off() {
        let mut ko = KnockoutState::new();
        ko.note_seen_from_tool_results(&[ok_chunks(&[ID_A, ID_B])]);
        ko.register_from_model_text(&format!("KNOCKOUT: {ID_A}"));
        let mut cap = json!({"chunks": [
            {"chunk_id": ID_A, "content": "noise"},
            {"chunk_id": ID_B, "content": "keep"},
        ]});
        ko.align_value_no_count(&mut cap);
        assert_eq!(cap["chunks"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn hard_filter_path_still_works_when_invoked_directly() {
        // Keep V1 algorithm under a private method for revival / regression.
        let mut ko = KnockoutState::new();
        ko.note_seen_from_tool_results(&[ok_chunks(&[ID_A, ID_B])]);
        ko.register_from_model_text(&format!("KNOCKOUT: {ID_A}"));
        let mut data = json!({"chunks": [
            {"chunk_id": ID_A, "content": "noise"},
            {"chunk_id": ID_B, "content": "keep"},
        ]});
        let re = ko.apply_to_bridge_data_hard(&mut data);
        assert!(re.is_empty());
        assert_eq!(data["chunks"].as_array().unwrap().len(), 1);
        assert_eq!(data["chunks"][0]["chunk_id"], ID_B);

        let mut data2 = json!({"chunks": [{"chunk_id": ID_A, "content": "n"}]});
        assert!(ko.apply_to_bridge_data_hard(&mut data2).is_empty());
        assert!(data2["chunks"].as_array().unwrap().is_empty());

        let mut data3 = json!({"chunks": [{"chunk_id": ID_A, "content": "n"}]});
        let re = ko.apply_to_bridge_data_hard(&mut data3);
        assert_eq!(re, vec![ID_A.to_string()]);
        assert_eq!(data3["chunks"].as_array().unwrap().len(), 1);
        assert_eq!(ko.take_reexposed(), vec![ID_A.to_string()]);
    }

    #[test]
    fn observability_reports_hard_suppress_off() {
        let mut ko = KnockoutState::new();
        ko.note_seen_from_tool_results(&[ok_chunks(&[ID_A, ID_B])]);
        ko.register_from_model_text(&format!("KNOCKOUT: {ID_A}"));
        let mut d = json!({"chunks": [{"chunk_id": ID_A, "content": "n"}]});
        assert!(ko.apply_to_bridge_data(&mut d).is_empty());
        let snap = ko.observability_snapshot();
        assert!(!snap.hard_suppress_enabled);
        assert_eq!(snap.registered_count, 1);
        assert_eq!(snap.registered_ids, vec![ID_A.to_string()]);
        assert_eq!(snap.seen_count, 2);
        assert_eq!(snap.suppress_events, 0);
        assert_eq!(snap.reexpose_events, 0);
    }
}
