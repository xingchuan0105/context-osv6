//! Persistable progress snapshot for assistant `turn_metadata.progress`.
//!
//! Shape is stable for the product API / frontend restore path:
//! ```json
//! {
//!   "progress": {
//!     "mode": "rag",
//!     "activities": [
//!       {
//!         "id": "act-0",
//!         "phase": "act:retrieve_semantic",
//!         "title": "progress.retrieve_semantic.done",
//!         "detail": "…",
//!         "counts": { "hits": 12 },
//!         "sources_preview": []
//!       }
//!     ],
//!     "collapsed": false
//!   }
//! }
//! ```

use crate::events::AgentEvent;
use serde_json::{json, Value};

/// Max reasoning chars stored in the snapshot (matches stream summary cap).
const REASONING_SUMMARY_MAX_CHARS: usize = 160;

/// Build assistant `turn_metadata` JSON containing a progress snapshot, if any
/// Activity / Reasoning events were observed.
pub fn assistant_progress_turn_metadata(agent_type: &str, events: &[AgentEvent]) -> Option<Value> {
    let mut activities: Vec<Value> = Vec::new();
    let mut reasoning = String::new();

    for event in events {
        match event {
            AgentEvent::Activity {
                stage,
                message,
                detail,
                counts,
                sources_preview,
            } => {
                let id = format!("act-{}", activities.len());
                let counts_obj: Value = counts
                    .iter()
                    .map(|(k, v)| (k.clone(), json!(v)))
                    .collect::<serde_json::Map<String, Value>>()
                    .into();
                let sources: Vec<Value> = sources_preview
                    .iter()
                    .map(|s| {
                        json!({
                            "id": s.id,
                            "label": s.label,
                            "href": s.href,
                        })
                    })
                    .collect();
                activities.push(json!({
                    "id": id,
                    "phase": stage,
                    "title": message,
                    "detail": detail,
                    "counts": counts_obj,
                    "sources_preview": sources,
                    "timestamp": Value::Null,
                }));
            }
            AgentEvent::ReasoningSummaryDelta { text } if !text.is_empty() => {
                reasoning.push_str(text);
            }
            _ => {}
        }
    }

    if !reasoning.is_empty() {
        let capped = crate::progress::truncate_chars(reasoning.trim(), REASONING_SUMMARY_MAX_CHARS);
        if !capped.is_empty() {
            activities.push(json!({
                "id": format!("reasoning-{}", activities.len()),
                "phase": "reasoning",
                "title": "progress.reasonPreview",
                "detail": capped,
                "counts": {},
                "sources_preview": [],
                "timestamp": Value::Null,
            }));
        }
    }

    if activities.is_empty() {
        return None;
    }

    let mode_owned = agent_type.to_ascii_lowercase();
    let mode = match mode_owned.as_str() {
        "general" => "chat",
        other => other,
    };

    Some(json!({
        "progress": {
            "mode": mode,
            "activities": activities,
            "collapsed": false,
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn builds_snapshot_from_activity_and_reasoning() {
        let events = vec![
            AgentEvent::Activity {
                stage: "act:retrieve_semantic".to_string(),
                message: "progress.retrieve_semantic.done".to_string(),
                detail: Some("模块".to_string()),
                counts: BTreeMap::from([("hits".to_string(), 3)]),
                sources_preview: vec![],
            },
            AgentEvent::ReasoningSummaryDelta {
                text: "先查语义再综合".to_string(),
            },
            AgentEvent::MessageDelta {
                text: "答案".to_string(),
            },
        ];
        let meta = assistant_progress_turn_metadata("rag", &events).expect("meta");
        let progress = &meta["progress"];
        assert_eq!(progress["mode"], "rag");
        assert_eq!(progress["activities"].as_array().unwrap().len(), 2);
        assert_eq!(
            progress["activities"][0]["title"],
            "progress.retrieve_semantic.done"
        );
        assert_eq!(progress["activities"][1]["phase"], "reasoning");
        assert_eq!(progress["activities"][1]["detail"], "先查语义再综合");
    }

    #[test]
    fn empty_events_yield_none() {
        assert!(assistant_progress_turn_metadata("chat", &[]).is_none());
        assert!(assistant_progress_turn_metadata(
            "chat",
            &[AgentEvent::MessageDelta {
                text: "hi".to_string()
            }]
        )
        .is_none());
    }
}
