use super::profile_types::{
    ObservedConflict, ProfileDelta, ProfileSingleton, ProfileSlot, SessionHint, SingletonUpdate,
    SlotUpdate, StoredConflict, StoredSessionHint, UserProfile,
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

fn normalize_evidence(evidence: &mut Vec<String>) {
    evidence.truncate(MAX_EVIDENCE_ITEMS);
    for item in evidence.iter_mut() {
        *item = truncate_text(item, MAX_EVIDENCE_LEN);
    }
}

fn signal_to_confidence(signal: &str) -> f64 {
    match signal {
        "strong" => 0.9,
        "medium" => 0.7,
        _ => 0.4,
    }
}

/// Decode stored jsonb blob → typed profile (malformed → empty profile, no panic).
pub fn user_profile_from_value(value: &serde_json::Value) -> UserProfile {
    if !value.is_object() {
        return UserProfile::default();
    }
    serde_json::from_value(value.clone()).unwrap_or_default()
}

/// Encode typed profile → jsonb for persistence.
pub fn user_profile_to_value(profile: &UserProfile) -> serde_json::Value {
    serde_json::to_value(profile).unwrap_or_else(|_| serde_json::json!({}))
}

/// Apply a semantic delta using deterministic merge rules (typed end-to-end).
pub fn apply_profile_delta(existing: UserProfile, delta: ProfileDelta) -> UserProfile {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let mut profile = existing;

    apply_slots(
        &mut profile.expertise_domains,
        &delta.expertise_domain_updates,
        5,
        &today,
        false,
    );
    apply_singleton(
        &mut profile.preferred_answer_style,
        delta.preferred_answer_style_update.as_ref(),
        &today,
    );
    apply_singleton(
        &mut profile.preferred_language,
        delta.preferred_language_update.as_ref(),
        &today,
    );
    apply_slots(
        &mut profile.tool_preferences,
        &delta.tool_preference_updates,
        3,
        &today,
        true,
    );
    apply_slots(
        &mut profile.important_constraints,
        &delta.important_constraint_updates,
        5,
        &today,
        false,
    );
    // Constraints: drop expired
    profile.important_constraints.retain(|s| {
        s.expires_at
            .as_deref()
            .map(|exp| exp >= today.as_str())
            .unwrap_or(true)
    });

    apply_hints(&mut profile.session_continuity_hints, &delta.session_continuity_hints, &today);
    apply_conflicts(&mut profile.observed_conflicts, &delta.observed_conflicts);

    if let Some(summary) = delta.global_summary.as_deref().filter(|s| !s.is_empty()) {
        profile.global_summary = Some(truncate_text(summary, 400));
    }

    profile
}

/// Storage-friendly wrapper: Value in/out around typed merge.
pub fn apply_profile_delta_value(
    existing: serde_json::Value,
    delta: ProfileDelta,
) -> serde_json::Value {
    let profile = user_profile_from_value(&existing);
    let merged = apply_profile_delta(profile, delta);
    user_profile_to_value(&merged)
}

fn slot_from_update(update: &SlotUpdate, base_confidence: f64, today: &str) -> ProfileSlot {
    let mut evidence = update.evidence.clone();
    normalize_evidence(&mut evidence);
    ProfileSlot {
        tag: update.tag.clone(),
        action: update.action.clone(),
        description: update
            .description
            .as_deref()
            .map(|d| truncate_text(d, MAX_DESCRIPTION_LEN)),
        reason: update
            .reason
            .as_deref()
            .map(|r| truncate_text(r, MAX_DESCRIPTION_LEN)),
        evidence,
        confidence_signal: update.confidence_signal.clone(),
        expires_at: update.expires_at.clone(),
        confidence: Some(base_confidence),
        since: Some(today.to_string()),
        last_seen: Some(today.to_string()),
    }
}

fn apply_slots(
    slots: &mut Vec<ProfileSlot>,
    updates: &[SlotUpdate],
    max_slots: usize,
    today: &str,
    validate_tool_tags: bool,
) {
    for update in updates {
        let action = update.action.as_deref().unwrap_or("none");
        let tag = update.tag.as_deref().unwrap_or("");
        if tag.is_empty() {
            continue;
        }
        if validate_tool_tags && !VALID_TOOL_TAGS.contains(&tag) {
            tracing::warn!(tag, "ignoring invalid tool_preference tag");
            continue;
        }

        let signal = update.confidence_signal.as_deref().unwrap_or("weak");
        let base_confidence = signal_to_confidence(signal);

        match action {
            "add" => {
                if slots.iter().any(|s| s.tag.as_deref() == Some(tag)) {
                    continue;
                }
                slots.push(slot_from_update(update, base_confidence, today));
            }
            "reinforce" | "revise" => {
                if let Some(existing) = slots.iter_mut().find(|s| s.tag.as_deref() == Some(tag)) {
                    let bump = if action == "reinforce" { 0.1 } else { 0.05 };
                    let old = existing.confidence.unwrap_or(0.5);
                    existing.confidence = Some((old + bump).min(0.95));
                    existing.last_seen = Some(today.to_string());
                    if let Some(desc) = update.description.as_deref().filter(|d| !d.is_empty()) {
                        existing.description = Some(truncate_text(desc, MAX_DESCRIPTION_LEN));
                    }
                }
            }
            "weaken" => {
                if let Some(existing) = slots.iter_mut().find(|s| s.tag.as_deref() == Some(tag)) {
                    let old = existing.confidence.unwrap_or(0.5);
                    existing.confidence = Some((old - 0.2).max(0.0));
                    existing.last_seen = Some(today.to_string());
                }
            }
            "remove" => {
                slots.retain(|s| s.tag.as_deref() != Some(tag));
            }
            _ => {}
        }
    }

    slots.retain(|s| s.confidence.map(|c| c >= 0.3).unwrap_or(true));

    if slots.len() > max_slots {
        slots.sort_by(|a, b| {
            let ca = a.confidence.unwrap_or(0.0);
            let cb = b.confidence.unwrap_or(0.0);
            cb.partial_cmp(&ca).unwrap_or(std::cmp::Ordering::Equal)
        });
        slots.truncate(max_slots);
    }
}

fn apply_singleton(
    existing: &mut Option<ProfileSingleton>,
    update: Option<&SingletonUpdate>,
    today: &str,
) {
    let update = match update {
        Some(u) if !u.is_noop() => u,
        _ => return,
    };
    let action = update.action.as_deref().unwrap_or("none");
    if action == "none" {
        return;
    }

    let signal = update.confidence_signal.as_deref().unwrap_or("weak");
    let base_confidence = signal_to_confidence(signal);
    let mut evidence = update.evidence.clone();
    normalize_evidence(&mut evidence);
    let description = update
        .description
        .as_deref()
        .map(|d| truncate_text(d, MAX_DESCRIPTION_LEN));

    match action {
        "set" => {
            *existing = Some(ProfileSingleton {
                tag: update.tag.clone(),
                action: update.action.clone(),
                description,
                evidence,
                confidence_signal: update.confidence_signal.clone(),
                value: update.value.clone(),
                confidence: Some(base_confidence),
                since: Some(today.to_string()),
            });
        }
        "reinforce" => {
            if let Some(cur) = existing.as_mut() {
                let old = cur.confidence.unwrap_or(0.5);
                cur.confidence = Some((old + 0.1).min(0.95));
                if let Some(desc) = description.filter(|d| !d.is_empty()) {
                    cur.description = Some(desc);
                }
            }
        }
        "revise" => {
            if let Some(cur) = existing.as_mut() {
                let old = cur.confidence.unwrap_or(0.5);
                cur.confidence = Some((old + 0.05).min(0.95));
                if let Some(desc) = description.filter(|d| !d.is_empty()) {
                    cur.description = Some(desc);
                }
                if let Some(tag) = update.tag.as_deref().filter(|t| !t.is_empty()) {
                    cur.tag = Some(tag.to_string());
                }
                if update.value.is_some() {
                    cur.value = update.value.clone();
                }
            }
        }
        "weaken" => {
            if let Some(cur) = existing.as_mut() {
                let old = cur.confidence.unwrap_or(0.5);
                cur.confidence = Some((old - 0.2).max(0.0));
            }
        }
        "clear" => {
            if let Some(cur) = existing.as_mut() {
                cur.value = None;
                cur.tag = None;
            }
        }
        _ => {}
    }

    if existing
        .as_ref()
        .and_then(|e| e.confidence)
        .map(|c| c < 0.3)
        .unwrap_or(false)
    {
        *existing = None;
    }
}

fn apply_hints(slots: &mut Vec<StoredSessionHint>, hints: &[SessionHint], today: &str) {
    for hint in hints {
        if let Some(priority) = hint.priority.as_deref() {
            if !VALID_HINT_PRIORITIES.contains(&priority) {
                tracing::warn!(priority, "ignoring invalid session_continuity_hints priority");
                continue;
            }
        }
        slots.push(StoredSessionHint {
            hint: hint
                .hint
                .as_deref()
                .map(|t| truncate_text(t, MAX_DESCRIPTION_LEN)),
            source_session_id: hint.source_session_id.clone(),
            priority: hint.priority.clone(),
            created_at: Some(today.to_string()),
        });
    }

    let cutoff = (chrono::NaiveDate::parse_from_str(today, "%Y-%m-%d")
        .unwrap_or_else(|_| chrono::Utc::now().date_naive())
        - chrono::Duration::days(7))
    .format("%Y-%m-%d")
    .to_string();
    slots.retain(|h| {
        h.created_at
            .as_deref()
            .map(|d| d >= cutoff.as_str())
            .unwrap_or(true)
    });
    if slots.len() > 3 {
        slots.truncate(3);
    }
}

fn apply_conflicts(existing: &mut Vec<StoredConflict>, new: &[ObservedConflict]) {
    for conflict in new {
        let c = StoredConflict {
            field: conflict.field.clone(),
            old_view: conflict.old_view.clone(),
            new_view: conflict.new_view.clone(),
            evidence: {
                let mut e = conflict.evidence.clone();
                normalize_evidence(&mut e);
                e
            },
        };
        let is_dup = existing.iter().any(|e| {
            e.field == c.field && e.old_view == c.old_view && e.new_view == c.new_view
        });
        if !is_dup {
            existing.push(c);
        }
    }
    if existing.len() > 10 {
        existing.truncate(10);
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

// --- Test helpers: JSON fixtures still exercise the same typed rules ---

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
    let mut p = user_profile_from_value(profile);
    let validate_tools = key == "tool_preferences";
    match key {
        "expertise_domains" => apply_slots(&mut p.expertise_domains, &typed, max_slots, today, false),
        "tool_preferences" => {
            apply_slots(&mut p.tool_preferences, &typed, max_slots, today, validate_tools)
        }
        "important_constraints" => {
            apply_slots(&mut p.important_constraints, &typed, max_slots, today, false);
            p.important_constraints.retain(|s| {
                s.expires_at
                    .as_deref()
                    .map(|exp| exp >= today)
                    .unwrap_or(true)
            });
        }
        _ => {}
    }
    *profile = user_profile_to_value(&p);
}

#[cfg(test)]
pub(crate) fn apply_singleton_update(
    profile: &mut serde_json::Value,
    key: &str,
    update: Option<&serde_json::Value>,
    today: &str,
) {
    let typed: Option<SingletonUpdate> =
        update.and_then(|v| serde_json::from_value(v.clone()).ok());
    let mut p = user_profile_from_value(profile);
    match key {
        "preferred_answer_style" => apply_singleton(&mut p.preferred_answer_style, typed.as_ref(), today),
        "preferred_language" => apply_singleton(&mut p.preferred_language, typed.as_ref(), today),
        _ => {}
    }
    *profile = user_profile_to_value(&p);
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
    let mut p = user_profile_from_value(profile);
    apply_hints(&mut p.session_continuity_hints, &typed, today);
    *profile = user_profile_to_value(&p);
}

#[cfg(test)]
pub(crate) fn apply_profile_delta_from_value(
    existing: serde_json::Value,
    delta: serde_json::Value,
) -> serde_json::Value {
    let typed: ProfileDelta = serde_json::from_value(delta).unwrap_or_default();
    apply_profile_delta_value(existing, typed)
}
