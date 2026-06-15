use super::profile_types::{
    ObservedConflict, ProfileDelta, SessionHint, SingletonUpdate, SlotUpdate,
};

pub const MAX_DESCRIPTION_LEN: usize = 200;
pub const MAX_EVIDENCE_LEN: usize = 200;
pub const MAX_EVIDENCE_ITEMS: usize = 5;
pub const VALID_TOOL_TAGS: &[&str] = &["rag", "search", "chat"];
pub const VALID_HINT_PRIORITIES: &[&str] = &["low", "medium", "high"];

pub fn truncate_text(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        s.chars().take(max_len).collect()
    } else {
        s.to_string()
    }
}

fn normalize_evidence(evidence: &mut Vec<serde_json::Value>) {
    evidence.truncate(MAX_EVIDENCE_ITEMS);
    for item in evidence.iter_mut() {
        if let Some(s) = item.as_str() {
            *item = serde_json::json!(truncate_text(s, MAX_EVIDENCE_LEN));
        }
    }
}

fn slot_update_to_value(update: &mut SlotUpdate) -> serde_json::Value {
    if let Some(desc) = update.description.as_deref() {
        update.description = Some(truncate_text(desc, MAX_DESCRIPTION_LEN));
    }
    if let Some(reason) = update.reason.as_deref() {
        update.reason = Some(truncate_text(reason, MAX_DESCRIPTION_LEN));
    }
    normalize_evidence(&mut update.evidence);
    serde_json::to_value(update).unwrap_or_else(|_| serde_json::json!({}))
}

fn singleton_update_to_value(update: &mut SingletonUpdate) -> serde_json::Value {
    if let Some(desc) = update.description.as_deref() {
        update.description = Some(truncate_text(desc, MAX_DESCRIPTION_LEN));
    }
    normalize_evidence(&mut update.evidence);
    serde_json::to_value(update).unwrap_or_else(|_| serde_json::json!({}))
}

fn ensure_profile_object(profile: &mut serde_json::Value) {
    if !profile.is_object() {
        *profile = serde_json::json!({});
    }
}

fn ensure_array_slot<'a>(
    profile: &'a mut serde_json::Value,
    key: &str,
) -> Option<&'a mut Vec<serde_json::Value>> {
    ensure_profile_object(profile);
    let obj = profile.as_object_mut()?;
    let slot = obj
        .entry(key.to_string())
        .or_insert_with(|| serde_json::json!([]));
    if !slot.is_array() {
        *slot = serde_json::json!([]);
    }
    slot.as_array_mut()
}

/// Apply a semantic delta to the existing profile using deterministic merge rules.
pub fn apply_profile_delta(existing: serde_json::Value, delta: ProfileDelta) -> serde_json::Value {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let mut profile = if existing.is_object() {
        existing
    } else {
        serde_json::json!({})
    };

    apply_slot_updates_from_typed(
        &mut profile,
        "expertise_domains",
        &delta.expertise_domain_updates,
        5,
        &today,
    );

    apply_singleton_update_from_typed(
        &mut profile,
        "preferred_answer_style",
        delta.preferred_answer_style_update.as_ref(),
        &today,
    );

    apply_singleton_update_from_typed(
        &mut profile,
        "preferred_language",
        delta.preferred_language_update.as_ref(),
        &today,
    );

    apply_slot_updates_from_typed(
        &mut profile,
        "tool_preferences",
        &delta.tool_preference_updates,
        3,
        &today,
    );

    apply_slot_updates_from_typed(
        &mut profile,
        "important_constraints",
        &delta.important_constraint_updates,
        5,
        &today,
    );

    apply_hint_updates_from_typed(&mut profile, &delta.session_continuity_hints, &today);

    if !delta.observed_conflicts.is_empty() {
        if let Some(existing_conflicts) =
            ensure_array_slot(&mut profile, "observed_conflicts")
        {
            for conflict in &delta.observed_conflicts {
                let c = conflict_to_value(conflict);
                let is_dup = existing_conflicts.iter().any(|e| {
                    e.get("field") == c.get("field")
                        && e.get("old_view") == c.get("old_view")
                        && e.get("new_view") == c.get("new_view")
                });
                if !is_dup {
                    existing_conflicts.push(c);
                }
            }
            if existing_conflicts.len() > 10 {
                existing_conflicts.truncate(10);
            }
        }
    }

    if let Some(summary) = delta.global_summary.as_deref().filter(|s| !s.is_empty()) {
        let summary = truncate_text(summary, 400);
        ensure_profile_object(&mut profile);
        if let Some(obj) = profile.as_object_mut() {
            obj.insert("global_summary".to_string(), serde_json::json!(summary));
        }
    }

    profile
}

fn conflict_to_value(conflict: &ObservedConflict) -> serde_json::Value {
    serde_json::json!({
        "field": conflict.field,
        "old_view": conflict.old_view,
        "new_view": conflict.new_view,
        "evidence": conflict.evidence,
    })
}

fn apply_slot_updates_from_typed(
    profile: &mut serde_json::Value,
    key: &str,
    updates: &[SlotUpdate],
    max_slots: usize,
    today: &str,
) {
    let slot_arr = match ensure_array_slot(profile, key) {
        Some(arr) => arr,
        None => return,
    };

    if !updates.is_empty() {
        for update in updates {
            let mut typed = update.clone();
            let update = slot_update_to_value(&mut typed);

            let action = update
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("none");
            let tag = update
                .get("tag")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if tag.is_empty() {
                continue;
            }

            if key == "tool_preferences" && !VALID_TOOL_TAGS.contains(&tag.as_str()) {
                tracing::warn!(tag, "ignoring invalid tool_preference tag");
                continue;
            }

            let signal = update
                .get("confidence_signal")
                .and_then(|v| v.as_str())
                .unwrap_or("weak");
            let base_confidence = signal_to_confidence(signal);

            match action {
                "add" => {
                    if slot_arr
                        .iter()
                        .any(|s| s.get("tag") == Some(&serde_json::json!(tag)))
                    {
                        continue;
                    }
                    let mut slot = update.clone();
                    slot["confidence"] = serde_json::json!(base_confidence);
                    slot["since"] = serde_json::json!(today);
                    slot["last_seen"] = serde_json::json!(today);
                    slot_arr.push(slot);
                }
                "reinforce" | "revise" => {
                    if let Some(existing) = slot_arr
                        .iter_mut()
                        .find(|s| s.get("tag") == Some(&serde_json::json!(tag)))
                    {
                        let bump = if action == "reinforce" { 0.1 } else { 0.05 };
                        let old_conf = existing
                            .get("confidence")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.5);
                        existing["confidence"] = serde_json::json!((old_conf + bump).min(0.95));
                        existing["last_seen"] = serde_json::json!(today);
                        if let Some(desc) = update.get("description").and_then(|v| v.as_str())
                            && !desc.is_empty()
                        {
                            existing["description"] = serde_json::json!(desc);
                        }
                    }
                }
                "weaken" => {
                    if let Some(existing) = slot_arr
                        .iter_mut()
                        .find(|s| s.get("tag") == Some(&serde_json::json!(tag)))
                    {
                        let old_conf = existing
                            .get("confidence")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.5);
                        existing["confidence"] = serde_json::json!((old_conf - 0.2).max(0.0));
                        existing["last_seen"] = serde_json::json!(today);
                    }
                }
                "remove" => {
                    slot_arr.retain(|s| s.get("tag") != Some(&serde_json::json!(tag)));
                }
                _ => {}
            }
        }
    }

    if key == "important_constraints" {
        slot_arr.retain(|s| {
            s.get("expires_at")
                .and_then(|v| v.as_str())
                .map(|exp| exp >= today)
                .unwrap_or(true)
        });
    }

    slot_arr.retain(|s| {
        s.get("confidence")
            .and_then(|v| v.as_f64())
            .map(|c| c >= 0.3)
            .unwrap_or(true)
    });

    if slot_arr.len() > max_slots {
        slot_arr.sort_by(|a, b| {
            let ca = a.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let cb = b.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.0);
            cb.partial_cmp(&ca).unwrap_or(std::cmp::Ordering::Equal)
        });
        slot_arr.truncate(max_slots);
    }
}

fn apply_singleton_update_from_typed(
    profile: &mut serde_json::Value,
    key: &str,
    update: Option<&SingletonUpdate>,
    today: &str,
) {
    let update = match update {
        Some(u) if !u.is_noop() => u,
        _ => return,
    };

    let mut typed = update.clone();
    let update = singleton_update_to_value(&mut typed);
    let action = update
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("none");
    if action == "none" {
        return;
    }

    let signal = update
        .get("confidence_signal")
        .and_then(|v| v.as_str())
        .unwrap_or("weak");
    let base_confidence = signal_to_confidence(signal);

    ensure_profile_object(profile);
    let existing = match profile.as_object_mut() {
        Some(obj) => obj
            .entry(key.to_string())
            .or_insert_with(|| serde_json::json!({})),
        None => return,
    };

    match action {
        "set" => {
            let mut new_val = update.clone();
            new_val["confidence"] = serde_json::json!(base_confidence);
            new_val["since"] = serde_json::json!(today);
            *existing = new_val;
        }
        "reinforce" => {
            let old_conf = existing
                .get("confidence")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.5);
            existing["confidence"] = serde_json::json!((old_conf + 0.1).min(0.95));
            if let Some(desc) = update.get("description").and_then(|v| v.as_str())
                && !desc.is_empty()
            {
                existing["description"] = serde_json::json!(desc);
            }
        }
        "revise" => {
            let old_conf = existing
                .get("confidence")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.5);
            existing["confidence"] = serde_json::json!((old_conf + 0.05).min(0.95));
            if let Some(desc) = update.get("description").and_then(|v| v.as_str())
                && !desc.is_empty()
            {
                existing["description"] = serde_json::json!(desc);
            }
            if let Some(tag) = update.get("tag").and_then(|v| v.as_str())
                && !tag.is_empty()
            {
                existing["tag"] = serde_json::json!(tag);
            }
            if let Some(val) = update.get("value") {
                existing["value"] = val.clone();
            }
        }
        "weaken" => {
            let old_conf = existing
                .get("confidence")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.5);
            existing["confidence"] = serde_json::json!((old_conf - 0.2).max(0.0));
        }
        "clear" => {
            existing["value"] = serde_json::Value::Null;
            existing["tag"] = serde_json::Value::Null;
        }
        _ => {}
    }

    if existing
        .get("confidence")
        .and_then(|v| v.as_f64())
        .map(|c| c < 0.3)
        .unwrap_or(false)
    {
        if let Some(obj) = profile.as_object_mut() {
            obj.remove(key);
        }
    }
}

fn apply_hint_updates_from_typed(
    profile: &mut serde_json::Value,
    hints: &[SessionHint],
    today: &str,
) {
    let slot_arr = match ensure_array_slot(profile, "session_continuity_hints") {
        Some(arr) => arr,
        None => return,
    };

    if !hints.is_empty() {
        for hint in hints {
            if let Some(priority) = hint.priority.as_deref() {
                if !VALID_HINT_PRIORITIES.contains(&priority) {
                    tracing::warn!(
                        priority,
                        "ignoring invalid session_continuity_hints priority"
                    );
                    continue;
                }
            }
            let mut h = serde_json::to_value(hint).unwrap_or_else(|_| serde_json::json!({}));
            if let Some(text) = hint.hint.as_deref() {
                h["hint"] = serde_json::json!(truncate_text(text, MAX_DESCRIPTION_LEN));
            }
            h["created_at"] = serde_json::json!(today);
            slot_arr.push(h);
        }
    }

    let cutoff = (chrono::NaiveDate::parse_from_str(today, "%Y-%m-%d")
        .unwrap_or_else(|_| chrono::Utc::now().date_naive())
        - chrono::Duration::days(7))
        .format("%Y-%m-%d")
        .to_string();
    slot_arr.retain(|h| {
        h.get("created_at")
            .and_then(|v| v.as_str())
            .map(|d| d >= cutoff.as_str())
            .unwrap_or(true)
    });

    if slot_arr.len() > 3 {
        slot_arr.truncate(3);
    }
}

fn signal_to_confidence(signal: &str) -> f64 {
    match signal {
        "strong" => 0.9,
        "medium" => 0.7,
        _ => 0.4,
    }
}

/// Parse LLM output into a typed profile delta; malformed shapes yield an empty delta (no panic).
pub fn parse_profile_delta_response(text: &str) -> ProfileDelta {
    let cleaned = text
        .trim()
        .strip_prefix("```json")
        .or_else(|| text.trim().strip_prefix("```"))
        .map(|s| s.trim())
        .and_then(|s| s.strip_suffix("```"))
        .map(|s| s.trim())
        .unwrap_or(text.trim());

    let value = serde_json::from_str::<serde_json::Value>(cleaned)
        .unwrap_or_else(|_| serde_json::json!({}));
    serde_json::from_value(value).unwrap_or_default()
}

// Legacy JSON-path helpers for unit tests that pass raw Value deltas.
#[cfg(test)]
pub(crate) fn apply_slot_updates(
    profile: &mut serde_json::Value,
    key: &str,
    updates: Option<&serde_json::Value>,
    max_slots: usize,
    today: &str,
) {
    let typed: Vec<SlotUpdate> = updates
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| serde_json::from_value(item.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    apply_slot_updates_from_typed(profile, key, &typed, max_slots, today);
}

#[cfg(test)]
pub(crate) fn apply_singleton_update(
    profile: &mut serde_json::Value,
    key: &str,
    update: Option<&serde_json::Value>,
    today: &str,
) {
    let typed: Option<SingletonUpdate> = update.and_then(|v| serde_json::from_value(v.clone()).ok());
    apply_singleton_update_from_typed(profile, key, typed.as_ref(), today);
}

#[cfg(test)]
pub(crate) fn apply_hint_updates(
    profile: &mut serde_json::Value,
    hints: Option<&serde_json::Value>,
    today: &str,
) {
    let typed: Vec<SessionHint> = hints
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| serde_json::from_value(item.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    apply_hint_updates_from_typed(profile, &typed, today);
}

#[cfg(test)]
pub(crate) fn apply_profile_delta_from_value(
    existing: serde_json::Value,
    delta: serde_json::Value,
) -> serde_json::Value {
    let typed: ProfileDelta = serde_json::from_value(delta).unwrap_or_default();
    apply_profile_delta(existing, typed)
}
