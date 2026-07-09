mod memory;
mod profile_merge;
mod profile_types;
mod quota;
mod visibility;

use app_core::NotificationCreateParams;
use common::AppError;

use crate::context::ChatContext;

impl ChatContext {
    pub(crate) async fn emit_notification(
        &self,
        event_type: &str,
        title: &str,
        body: &str,
        data: serde_json::Value,
    ) -> Result<(), AppError> {
        let Some(pg) = self.storage.chat_persistence() else {
            return Ok(());
        };
        let Some(user_id) = self.auth.actor_id().map(|value| value.into_uuid()) else {
            return Ok(());
        };
        pg.create_notification(
            &self.auth,
            NotificationCreateParams {
                user_id,
                event_type: event_type.to_string(),
                title: title.to_string(),
                body: body.to_string(),
                data,
                channels: vec!["in_app".to_string()],
            },
        )
        .await?;
        Ok(())
    }

    /// Record LLM token usage into the usage-limit metering service.
    /// Silently no-ops if the service is not configured.
    pub(crate) async fn record_llm_usage_if_available(
        &self,
        feature: avrag_billing::usage_limit::BillableFeature,
        stage: &str,
        usage: &avrag_llm::LlmUsage,
        source: &str,
    ) {
        let analytics_ctx = self.analytics_ctx();
        self.billing
            .record_llm_usage(&self.auth, &analytics_ctx, feature, stage, usage, source)
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::profile_merge::{
        MAX_DESCRIPTION_LEN, MAX_EVIDENCE_ITEMS, MAX_EVIDENCE_LEN, apply_hint_updates,
        apply_profile_delta_from_value, apply_profile_delta_value, apply_singleton_update,
        apply_slot_updates, parse_profile_delta_response, truncate_text,
    };
    use super::profile_types::ProfileDelta;

    fn slot(tag: &str, action: &str, signal: &str, confidence: f64) -> serde_json::Value {
        serde_json::json!({
            "tag": tag,
            "action": action,
            "description": "desc",
            "evidence": ["ev"],
            "confidence_signal": signal,
            "confidence": confidence,
            "since": "2026-01-01",
            "last_seen": "2026-01-01"
        })
    }

    #[test]
    fn slot_add_creates_with_base_confidence() {
        let mut profile = serde_json::json!({"expertise_domains": []});
        let delta = serde_json::json!([{
            "tag": "rust",
            "action": "add",
            "description": "desc",
            "evidence": ["ev"],
            "confidence_signal": "strong"
        }]);
        apply_slot_updates(
            &mut profile,
            "expertise_domains",
            Some(&delta),
            5,
            "2026-06-06",
        );
        let arr = profile["expertise_domains"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["confidence"], 0.9);
        assert_eq!(arr[0]["since"], "2026-06-06");
    }

    #[test]
    fn slot_reinforce_bumps_confidence_by_01() {
        let mut profile =
            serde_json::json!({"expertise_domains": [slot("rust", "add", "medium", 0.7)]});
        let delta = serde_json::json!([{
            "tag": "rust",
            "action": "reinforce",
            "description": "desc2",
            "evidence": ["ev2"],
            "confidence_signal": "medium"
        }]);
        apply_slot_updates(
            &mut profile,
            "expertise_domains",
            Some(&delta),
            5,
            "2026-06-06",
        );
        let arr = profile["expertise_domains"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert!((arr[0]["confidence"].as_f64().unwrap() - 0.8).abs() < 0.001);
        assert_eq!(arr[0]["description"], "desc2");
    }

    #[test]
    fn slot_revise_bumps_confidence_by_005() {
        let mut profile =
            serde_json::json!({"expertise_domains": [slot("rust", "add", "medium", 0.7)]});
        let delta = serde_json::json!([{
            "tag": "rust",
            "action": "revise",
            "description": "desc3",
            "evidence": ["ev3"],
            "confidence_signal": "medium"
        }]);
        apply_slot_updates(
            &mut profile,
            "expertise_domains",
            Some(&delta),
            5,
            "2026-06-06",
        );
        let arr = profile["expertise_domains"].as_array().unwrap();
        assert!((arr[0]["confidence"].as_f64().unwrap() - 0.75).abs() < 0.001);
    }

    #[test]
    fn slot_weaken_drops_confidence_by_02() {
        let mut profile =
            serde_json::json!({"expertise_domains": [slot("rust", "add", "medium", 0.7)]});
        let delta = serde_json::json!([{
            "tag": "rust",
            "action": "weaken",
            "evidence": ["ev"],
            "confidence_signal": "weak"
        }]);
        apply_slot_updates(
            &mut profile,
            "expertise_domains",
            Some(&delta),
            5,
            "2026-06-06",
        );
        let arr = profile["expertise_domains"].as_array().unwrap();
        assert!((arr[0]["confidence"].as_f64().unwrap() - 0.5).abs() < 0.001);
    }

    #[test]
    fn slot_evicts_below_03_threshold() {
        let mut profile =
            serde_json::json!({"expertise_domains": [slot("rust", "add", "medium", 0.35)]});
        let delta = serde_json::json!([{
            "tag": "rust",
            "action": "weaken",
            "evidence": ["ev"],
            "confidence_signal": "weak"
        }]);
        apply_slot_updates(
            &mut profile,
            "expertise_domains",
            Some(&delta),
            5,
            "2026-06-06",
        );
        let arr = profile["expertise_domains"].as_array().unwrap();
        assert!(arr.is_empty());
    }

    #[test]
    fn slot_evicts_excess_by_confidence() {
        let mut profile = serde_json::json!({
            "expertise_domains": [
                slot("a", "add", "weak", 0.4),
                slot("b", "add", "weak", 0.5),
                slot("c", "add", "weak", 0.6)
            ]
        });
        let delta = serde_json::json!([{
            "tag": "d",
            "action": "add",
            "description": "desc",
            "evidence": ["ev"],
            "confidence_signal": "strong"
        }]);
        apply_slot_updates(
            &mut profile,
            "expertise_domains",
            Some(&delta),
            3,
            "2026-06-06",
        );
        let arr = profile["expertise_domains"].as_array().unwrap();
        assert_eq!(arr.len(), 3);
        let tags: Vec<&str> = arr.iter().map(|s| s["tag"].as_str().unwrap()).collect();
        assert_eq!(tags, vec!["d", "c", "b"]);
    }

    #[test]
    fn slot_expires_constraints_by_expires_at() {
        let mut profile = serde_json::json!({
            "important_constraints": [
                serde_json::json!({
                    "tag": "old",
                    "description": "old",
                    "confidence": 0.7,
                    "since": "2026-01-01",
                    "last_seen": "2026-01-01",
                    "expires_at": "2026-01-01"
                })
            ]
        });
        let delta = serde_json::json!([]);
        apply_slot_updates(
            &mut profile,
            "important_constraints",
            Some(&delta),
            5,
            "2026-06-06",
        );
        let arr = profile["important_constraints"].as_array().unwrap();
        assert!(arr.is_empty());
    }

    #[test]
    fn slot_ignores_invalid_tool_tag() {
        let mut profile = serde_json::json!({"tool_preferences": []});
        let delta = serde_json::json!([{
            "tag": "invalid_tool",
            "action": "add",
            "reason": "r",
            "evidence": ["ev"],
            "confidence_signal": "strong"
        }]);
        apply_slot_updates(
            &mut profile,
            "tool_preferences",
            Some(&delta),
            3,
            "2026-06-06",
        );
        let arr = profile["tool_preferences"].as_array().unwrap();
        assert!(arr.is_empty());
    }

    #[test]
    fn slot_accepts_valid_tool_tag() {
        let mut profile = serde_json::json!({"tool_preferences": []});
        let delta = serde_json::json!([{
            "tag": "rag",
            "action": "add",
            "reason": "r",
            "evidence": ["ev"],
            "confidence_signal": "strong"
        }]);
        apply_slot_updates(
            &mut profile,
            "tool_preferences",
            Some(&delta),
            3,
            "2026-06-06",
        );
        let arr = profile["tool_preferences"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["tag"], "rag");
    }

    #[test]
    fn slot_truncates_description_and_evidence() {
        let mut profile = serde_json::json!({"expertise_domains": []});
        let long = "x".repeat(500);
        let delta = serde_json::json!([{
            "tag": "rust",
            "action": "add",
            "description": &long,
            "evidence": [&long, &long, &long, &long, &long, &long],
            "confidence_signal": "strong"
        }]);
        apply_slot_updates(
            &mut profile,
            "expertise_domains",
            Some(&delta),
            5,
            "2026-06-06",
        );
        let arr = profile["expertise_domains"].as_array().unwrap();
        assert_eq!(
            arr[0]["description"].as_str().unwrap().chars().count(),
            MAX_DESCRIPTION_LEN
        );
        let ev = arr[0]["evidence"].as_array().unwrap();
        assert_eq!(ev.len(), MAX_EVIDENCE_ITEMS);
        assert_eq!(ev[0].as_str().unwrap().chars().count(), MAX_EVIDENCE_LEN);
    }

    #[test]
    fn singleton_set_creates_with_base_confidence() {
        let mut profile = serde_json::json!({});
        let delta = serde_json::json!({
            "tag": "concise-writing",
            "action": "set",
            "description": "desc",
            "evidence": ["ev"],
            "confidence_signal": "strong"
        });
        apply_singleton_update(
            &mut profile,
            "preferred_answer_style",
            Some(&delta),
            "2026-06-06",
        );
        assert_eq!(profile["preferred_answer_style"]["confidence"], 0.9);
        assert_eq!(profile["preferred_answer_style"]["tag"], "concise-writing");
    }

    #[test]
    fn singleton_reinforce_bumps_by_01() {
        let mut profile = serde_json::json!({
            "preferred_answer_style": {
                "tag": "concise-writing", "confidence": 0.7, "since": "2026-01-01"
            }
        });
        let delta = serde_json::json!({
            "action": "reinforce",
            "evidence": ["ev"],
            "confidence_signal": "medium"
        });
        apply_singleton_update(
            &mut profile,
            "preferred_answer_style",
            Some(&delta),
            "2026-06-06",
        );
        assert!(
            (profile["preferred_answer_style"]["confidence"]
                .as_f64()
                .unwrap()
                - 0.8)
                .abs()
                < 0.001
        );
    }

    #[test]
    fn singleton_weaken_evicts_below_threshold() {
        let mut profile = serde_json::json!({
            "preferred_answer_style": {
                "tag": "concise-writing", "confidence": 0.35, "since": "2026-01-01"
            }
        });
        let delta = serde_json::json!({
            "action": "weaken",
            "evidence": ["ev"],
            "confidence_signal": "weak"
        });
        apply_singleton_update(
            &mut profile,
            "preferred_answer_style",
            Some(&delta),
            "2026-06-06",
        );
        assert!(
            profile
                .as_object()
                .unwrap()
                .get("preferred_answer_style")
                .is_none()
        );
    }

    #[test]
    fn hint_caps_at_three_fifo() {
        let mut profile = serde_json::json!({"session_continuity_hints": []});
        let delta = serde_json::json!([
            {"hint": "first", "source_session_id": "s1", "priority": "low"},
            {"hint": "second", "source_session_id": "s2", "priority": "medium"},
            {"hint": "third", "source_session_id": "s3", "priority": "high"},
            {"hint": "fourth", "source_session_id": "s4", "priority": "low"}
        ]);
        apply_hint_updates(&mut profile, Some(&delta), "2026-06-06");
        let arr = profile["session_continuity_hints"].as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0]["hint"], "first");
        assert_eq!(arr[2]["hint"], "third");
    }

    #[test]
    fn hint_expires_after_seven_days() {
        let mut profile = serde_json::json!({
            "session_continuity_hints": [
                {"hint": "old", "source_session_id": "s0", "priority": "low", "created_at": "2026-05-01"}
            ]
        });
        let delta = serde_json::json!([]);
        apply_hint_updates(&mut profile, Some(&delta), "2026-06-06");
        let arr = profile["session_continuity_hints"].as_array().unwrap();
        assert!(arr.is_empty());
    }

    #[test]
    fn hint_ignores_invalid_priority() {
        let mut profile = serde_json::json!({"session_continuity_hints": []});
        let delta = serde_json::json!([
            {"hint": "valid", "source_session_id": "s1", "priority": "urgent"}
        ]);
        apply_hint_updates(&mut profile, Some(&delta), "2026-06-06");
        let arr = profile["session_continuity_hints"].as_array().unwrap();
        assert!(arr.is_empty());
    }

    #[test]
    fn profile_delta_dedupes_conflicts() {
        let profile = serde_json::json!({
            "expertise_domains": [],
            "tool_preferences": [],
            "important_constraints": [],
            "session_continuity_hints": [],
            "observed_conflicts": [{
                "field": "preferred_language",
                "old_view": "en",
                "new_view": "zh",
                "evidence": ["old"]
            }]
        });
        let delta = serde_json::json!({
            "expertise_domain_updates": [],
            "preferred_answer_style_update": {"action": "none", "confidence_signal": "weak"},
            "preferred_language_update": {"action": "none", "confidence_signal": "weak"},
            "tool_preference_updates": [],
            "important_constraint_updates": [],
            "session_continuity_hints": [],
            "observed_conflicts": [
                {"field": "preferred_language", "old_view": "en", "new_view": "zh", "evidence": ["new"]},
                {"field": "preferred_style", "old_view": "concise", "new_view": "detailed", "evidence": ["new2"]}
            ],
            "global_summary": "summary"
        });
        let merged = apply_profile_delta_from_value(profile, delta);
        let conflicts = merged["observed_conflicts"].as_array().unwrap();
        assert_eq!(conflicts.len(), 2);
        assert_eq!(conflicts[0]["field"], "preferred_language");
        assert_eq!(conflicts[1]["field"], "preferred_style");
    }

    #[test]
    fn truncate_text_respects_char_boundaries() {
        let s = "a".repeat(300);
        assert_eq!(truncate_text(&s, 200).chars().count(), 200);
        let short = "hello";
        assert_eq!(truncate_text(short, 200), "hello");
    }

    #[test]
    fn malformed_llm_json_profile_not_object_does_not_panic() {
        let delta = parse_profile_delta_response("not-json-at-all");
        assert!(delta.is_effectively_empty());

        let merged = apply_profile_delta_value(serde_json::json!("not-an-object"), delta);
        assert!(merged.is_object());
    }

    #[test]
    fn malformed_llm_json_slot_not_array_does_not_panic() {
        let raw = r#"{"expertise_domain_updates": "not-an-array", "tool_preference_updates": 42}"#;
        let delta = parse_profile_delta_response(raw);
        assert!(delta.expertise_domain_updates.is_empty());
        assert!(delta.tool_preference_updates.is_empty());

        let existing = serde_json::json!({
            "expertise_domains": "also-not-array",
            "tool_preferences": {"tag": "rag"}
        });
        let merged = apply_profile_delta_value(existing, delta);
        assert!(merged["expertise_domains"].is_array());
        assert!(merged["tool_preferences"].is_array());
    }

    #[test]
    fn typed_profile_delta_merge_with_malformed_existing_profile() {
        let delta: ProfileDelta = serde_json::from_value(serde_json::json!({
            "expertise_domain_updates": [{
                "tag": "rust",
                "action": "add",
                "description": "desc",
                "evidence": ["ev"],
                "confidence_signal": "strong"
            }]
        }))
        .expect("valid delta");
        let merged = apply_profile_delta_value(serde_json::Value::Null, delta);
        let arr = merged["expertise_domains"].as_array().expect("array slot");
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["tag"], "rust");
    }
}
