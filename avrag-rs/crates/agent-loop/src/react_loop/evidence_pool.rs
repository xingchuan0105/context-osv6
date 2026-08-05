//! Run-scoped durable Evidence Pool + Intake (architecture deepen W2 / review I1–I2).
//!
//! Durable: alias namespace, full bodies, model-surfaced alias set, claim board.
//! Bridge writes aliases/bodies through shared Arcs; Intake merges Ok retrieval
//! JSON into the claim board at the codegen result boundary (not inside
//! observation string builders).

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use contracts::ToolStatus;

use super::claim_notes::{self, ClaimNoteLine};
use super::deps::BridgeCallObs;

/// Methods whose Ok payloads feed the claim board / retrieval summary.
pub const RETRIEVAL_INTAKE_METHODS: &[&str] = &["dense", "lexical", "grep", "web", "fetch"];

/// Run-owned durable evidence memory (Messenger host side).
#[derive(Clone)]
pub struct EvidencePool {
    pub seen_chunk_aliases: Arc<Mutex<HashMap<String, String>>>,
    pub seen_chunk_bodies: Arc<Mutex<HashMap<String, String>>>,
    /// Aliases already reported in prior-round model-visible summaries.
    pub seen_retrieval_aliases: Arc<Mutex<HashSet<String>>>,
    pub claim_notes: Vec<ClaimNoteLine>,
}

impl EvidencePool {
    pub fn new() -> Self {
        Self {
            seen_chunk_aliases: Arc::new(Mutex::new(HashMap::new())),
            seen_chunk_bodies: Arc::new(Mutex::new(HashMap::new())),
            seen_retrieval_aliases: Arc::new(Mutex::new(HashSet::new())),
            claim_notes: Vec::new(),
        }
    }

    /// Full text by chunk_id when bridge shared body store is populated.
    pub fn body_for_chunk(&self, chunk_id: &str) -> Option<String> {
        self.seen_chunk_bodies
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(chunk_id)
            .cloned()
    }

    /// Intake at Ok-retrieval boundary: expanded hits → claim board.
    pub fn intake_from_bridge_calls(&mut self, bridge_calls: &[BridgeCallObs]) {
        let datas: Vec<&serde_json::Value> = bridge_calls
            .iter()
            .filter(|c| c.result.status == ToolStatus::Ok)
            .filter(|c| RETRIEVAL_INTAKE_METHODS.contains(&c.method.as_str()))
            .filter_map(|c| c.result.data.as_ref())
            .collect();
        claim_notes::accumulate_from_tool_datas(&mut self.claim_notes, datas);
    }

    /// Intake from raw tool JSON payloads (tests / non-bridge paths).
    pub fn intake_tool_datas<'a, I>(&mut self, datas: I)
    where
        I: IntoIterator<Item = &'a serde_json::Value>,
    {
        claim_notes::accumulate_from_tool_datas(&mut self.claim_notes, datas);
    }
}

impl Default for EvidencePool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn body_lookup_and_expanded_claim_intake() {
        let mut pool = EvidencePool::new();
        {
            let mut bodies = pool.seen_chunk_bodies.lock().unwrap();
            bodies.insert("c1".into(), "full text".into());
        }
        assert_eq!(pool.body_for_chunk("c1").as_deref(), Some("full text"));

        // Expanded, non-omitted, long enough → claim line.
        pool.intake_tool_datas([json!({
            "chunks": [{
                "alias": "#1",
                "chunk_id": "c1",
                "visibility": "expanded",
                "text": "Important fact about the widget pricing schedule for 2024.",
            }]
        })]
        .iter());
        assert_eq!(pool.claim_notes.len(), 1);
        assert_eq!(pool.claim_notes[0].alias, "#1");
        assert!(pool.claim_notes[0].excerpt.contains("Important fact"));

        // Card / body_omitted → skip.
        let before = pool.claim_notes.len();
        pool.intake_tool_datas([json!({
            "chunks": [{
                "alias": "#2",
                "chunk_id": "c2",
                "visibility": "card",
                "body_omitted": true,
                "text": "snippet only",
            }]
        })]
        .iter());
        assert_eq!(pool.claim_notes.len(), before);
    }
}
