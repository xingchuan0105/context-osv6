use avrag_rag_core::context::SessionContext as RagSessionContext;
use app_core::{ChatPersistencePort, NotificationCreateParams};
use app_core::{MemoryState, StoredDocument};
use common::{AppError, ChatMessage, ChatSession, DocumentStatus};

use crate::context::ChatContext;

impl ChatContext {
    pub(crate) async fn list_ready_documents_for_chat(
        &self,
        notebook_id: &str,
        doc_scope: &[String],
    ) -> Vec<StoredDocument> {
        let state = self.storage.inner().read().await;
        state
            .documents
            .values()
            .filter(|stored| stored.document.notebook_id == notebook_id)
            .filter(|stored| matches!(stored.document.status, DocumentStatus::Completed))
            .filter(|stored| doc_scope.is_empty() || doc_scope.contains(&stored.document.id))
            .cloned()
            .collect()
    }

    pub fn build_rag_session_context(
        messages: Vec<ChatMessage>,
        summary: Option<String>,
    ) -> Option<RagSessionContext> {
        let summary = summary
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if messages.is_empty() && summary.is_none() {
            None
        } else {
            Some(RagSessionContext { messages, summary })
        }
    }

    /// The "dream" layer: runs at most once per day per user.
    /// LLM proposes a semantic delta; runtime applies deterministic merge rules.
    pub(crate) async fn maybe_update_structured_profile(
        &self,
        chat_persistence: &dyn ChatPersistencePort,
        _session: &ChatSession,
        recent_turns: &str,
    ) -> bool {
        let Some(cm) = &self.orchestrator.chatmemory() else {
            return false;
        };
        let Some(user_id) = self.auth.actor_id().map(|value| value.into_uuid()) else {
            return false;
        };

        let existing_profile = chat_persistence
            .get_user_profile(&self.auth, user_id)
            .await
            .ok()
            .flatten();

        let should_dream = match &existing_profile {
            Some(profile) => {
                let since_last = chrono::Utc::now().signed_duration_since(profile.inferred_at);
                since_last.num_hours() >= 24
            }
            None => true,
        };
        if !should_dream {
            return false;
        }

        let existing_structured = existing_profile
            .as_ref()
            .map(|p| p.structured_profile.clone())
            .unwrap_or_else(|| serde_json::json!({}));

        let delta = self
            .infer_profile_delta(recent_turns, &existing_structured)
            .await;
        if delta.is_null() || delta.as_object().map(|o| o.is_empty()).unwrap_or(false) {
            return false;
        }

        let merged = apply_profile_delta(existing_structured, delta);

        let expertise_domains = existing_profile
            .as_ref()
            .map(|p| p.expertise_domains.clone())
            .unwrap_or_default();
        let preferred_answer_style = existing_profile
            .as_ref()
            .and_then(|p| p.preferred_answer_style.clone());
        let frequently_asked_topics = existing_profile
            .as_ref()
            .map(|p| p.frequently_asked_topics.clone())
            .unwrap_or_default();
        let custom_preferences = existing_profile
            .as_ref()
            .map(|p| p.custom_preferences.clone())
            .unwrap_or_else(|| serde_json::json!({}));

        cm.update_user_profile(
            &self.auth,
            avrag_chatmemory::UserProfileUpdate {
                expertise_domains,
                preferred_answer_style,
                frequently_asked_topics,
                custom_preferences,
                structured_profile: merged,
                inference_version: "dream-v2".to_string(),
            },
        )
        .await
        .is_ok()
    }

    /// Ask the LLM to produce a semantic delta, not a full profile.
    pub(crate) async fn infer_profile_delta(
        &self,
        recent_turns: &str,
        existing_profile: &serde_json::Value,
    ) -> serde_json::Value {
        const DREAM_SYSTEM_PROMPT: &str =
            include_str!("../../../prompts/pipeline/user-profile-extraction.system.md");
        let system_prompt = DREAM_SYSTEM_PROMPT.trim();
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let user_prompt = format!(
            "Today's date: {}\n\nExisting profile:\n{}\n\nRecent conversation turns:\n{}",
            today,
            serde_json::to_string_pretty(existing_profile).unwrap_or_else(|_| "{}".to_string()),
            recent_turns.trim()
        );

        for (llm, temperature) in [
            (self.llm_ctx.memory_client(), self.llm_ctx.memory_llm_temperature()),
            (self.llm_ctx.agent_client(), self.llm_ctx.agent_llm_temperature()),
        ] {
            if let Some(client) = llm
                && let Ok(response) = client
                    .complete(
                        &[
                            avrag_llm::ChatMessage::system(system_prompt),
                            avrag_llm::ChatMessage::user(&user_prompt),
                        ],
                        temperature,
                    )
                    .await
            {
                let trimmed = response.content.trim();
                if !trimmed.is_empty() {
                    // Run output guards before parsing; block prompt leaks and scrub PII
                    // before any profile mutation.
                    let (sanitized, guard_report) = self.orchestrator.guard_pipeline().check_output(trimmed, None);
                    if guard_report.blocked {
                        tracing::warn!(
                            "dream layer output blocked by guard pipeline: {:?}",
                            guard_report.degrade_trace
                        );
                        return serde_json::json!({});
                    }
                    self.record_llm_usage_if_available(
                        avrag_billing::usage_limit::BillableFeature::Summary,
                        "dream_delta",
                        &response.usage,
                        "inline",
                    )
                    .await;
                    return parse_structured_json_response(&sanitized);
                }
            }
        }
        serde_json::json!({})
    }

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

    /// Get usage limit response for the current user.
    pub async fn get_user_usage_limit(
        &self,
    ) -> Result<avrag_billing::usage_limit::UsageLimitResponse, AppError> {
        self.billing.get_user_usage_limit(&self.auth).await
    }

    /// Check if the current user has quota remaining.
    pub async fn check_user_quota(
        &self,
    ) -> Result<avrag_billing::usage_limit::QuotaCheckResult, AppError> {
        self.billing.check_user_quota(&self.auth).await
    }

    pub(crate) async fn ensure_metric_quota(
        &self,
        metric_type: &str,
        requested: i64,
    ) -> Result<(), AppError> {
        self.billing
            .ensure_metric_quota(&self.auth, metric_type, requested)
            .await
    }

    pub(crate) async fn record_usage(
        &self,
        metric_type: &str,
        quantity: i64,
        source: &str,
    ) -> Result<(), AppError> {
        if quantity <= 0 {
            return Ok(());
        }
        let Some(pg) = self.storage.chat_persistence() else {
            return Ok(());
        };
        pg.record_usage_event(&self.auth, metric_type, quantity, source).await?;
        Ok(())
    }

    pub(crate) fn current_org_id(&self) -> String {
        self.auth.org_id().to_string()
    }

    pub fn memory_session_visible(
        &self,
        state: &MemoryState,
        session: &ChatSession,
    ) -> bool {
        state
            .notebooks
            .get(&session.notebook_id)
            .map(|notebook| notebook.org_id == self.current_org_id())
            .unwrap_or(false)
    }
}

/// Parse a structured JSON response into a serde_json::Value.
/// Strips markdown code fences if present.
fn parse_structured_json_response(text: &str) -> serde_json::Value {
    let cleaned = text
        .trim()
        .strip_prefix("```json")
        .or_else(|| text.trim().strip_prefix("```"))
        .map(|s| s.trim())
        .and_then(|s| s.strip_suffix("```"))
        .map(|s| s.trim())
        .unwrap_or(text.trim());

    serde_json::from_str(cleaned).unwrap_or_else(|_| serde_json::json!({}))
}

/// Apply a semantic delta to the existing profile using deterministic merge rules.
/// This is the runtime merge layer — no LLM involved.
fn apply_profile_delta(existing: serde_json::Value, delta: serde_json::Value) -> serde_json::Value {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let mut profile = if existing.is_object() {
        existing
    } else {
        serde_json::json!({})
    };

    // ── expertise_domains ──
    apply_slot_updates(
        &mut profile,
        "expertise_domains",
        delta.get("expertise_domain_updates"),
        5,
        &today,
    );

    // ── preferred_answer_style ──
    apply_singleton_update(
        &mut profile,
        "preferred_answer_style",
        delta.get("preferred_answer_style_update"),
        &today,
    );

    // ── preferred_language ──
    apply_singleton_update(
        &mut profile,
        "preferred_language",
        delta.get("preferred_language_update"),
        &today,
    );

    // ── tool_preferences ──
    apply_slot_updates(
        &mut profile,
        "tool_preferences",
        delta.get("tool_preference_updates"),
        3,
        &today,
    );

    // ── important_constraints ──
    apply_slot_updates(
        &mut profile,
        "important_constraints",
        delta.get("important_constraint_updates"),
        5,
        &today,
    );

    // ── session_continuity_hints ──
    apply_hint_updates(&mut profile, delta.get("session_continuity_hints"), &today);

    // ── observed_conflicts ──
    if let Some(conflicts) = delta.get("observed_conflicts").and_then(|v| v.as_array()) {
        let arr = profile
            .as_object_mut()
            .unwrap()
            .entry("observed_conflicts")
            .or_insert_with(|| serde_json::json!([]));
        if let Some(existing) = arr.as_array_mut() {
            for c in conflicts {
                // Dedupe by (field, old_view, new_view) triple to avoid accumulation
                // of duplicate contradictions across dream runs.
                let is_dup = existing.iter().any(|e| {
                    e.get("field") == c.get("field")
                        && e.get("old_view") == c.get("old_view")
                        && e.get("new_view") == c.get("new_view")
                });
                if !is_dup {
                    existing.push(c.clone());
                }
            }
            // Keep last 10 conflicts
            if existing.len() > 10 {
                existing.truncate(10);
            }
        }
    }

    // ── global_summary ──
    if let Some(summary) = delta.get("global_summary").and_then(|v| v.as_str())
        && !summary.is_empty()
    {
        let summary = truncate_text(summary, 400);
        profile
            .as_object_mut()
            .unwrap()
            .insert("global_summary".to_string(), serde_json::json!(summary));
    }

    profile
}

const MAX_DESCRIPTION_LEN: usize = 200;
const MAX_EVIDENCE_LEN: usize = 200;
const MAX_EVIDENCE_ITEMS: usize = 5;
const VALID_TOOL_TAGS: &[&str] = &["rag", "search", "chat"];
const VALID_HINT_PRIORITIES: &[&str] = &["low", "medium", "high"];

fn truncate_text(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        s.chars().take(max_len).collect()
    } else {
        s.to_string()
    }
}

fn normalize_evidence(evidence: &mut serde_json::Value) {
    if let Some(arr) = evidence.as_array_mut() {
        arr.truncate(MAX_EVIDENCE_ITEMS);
        for item in arr.iter_mut() {
            if let Some(s) = item.as_str() {
                *item = serde_json::json!(truncate_text(s, MAX_EVIDENCE_LEN));
            }
        }
    }
}

fn normalize_slot_update(update: &mut serde_json::Value) {
    if let Some(desc) = update.get("description").and_then(|v| v.as_str()) {
        update["description"] = serde_json::json!(truncate_text(desc, MAX_DESCRIPTION_LEN));
    }
    if let Some(reason) = update.get("reason").and_then(|v| v.as_str()) {
        update["reason"] = serde_json::json!(truncate_text(reason, MAX_DESCRIPTION_LEN));
    }
    if let Some(evidence) = update.get_mut("evidence") {
        normalize_evidence(evidence);
    }
}

/// Merge slot-array updates (expertise_domains, tool_preferences, constraints).
fn apply_slot_updates(
    profile: &mut serde_json::Value,
    key: &str,
    updates: Option<&serde_json::Value>,
    max_slots: usize,
    today: &str,
) {
    let slots = profile
        .as_object_mut()
        .unwrap()
        .entry(key)
        .or_insert_with(|| serde_json::json!([]));
    let slot_arr = slots.as_array_mut().unwrap();

    if let Some(updates) = updates.and_then(|v| v.as_array()).filter(|a| !a.is_empty()) {
        for update in updates {
            let mut update = update.clone();
            normalize_slot_update(&mut update);

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

    // Always run expiration and eviction, even if no new updates arrived.
    if key == "important_constraints" {
        slot_arr.retain(|s| {
            s.get("expires_at")
                .and_then(|v| v.as_str())
                .map(|exp| exp >= today)
                .unwrap_or(true)
        });
    }

    // Evict low-confidence slots
    slot_arr.retain(|s| {
        s.get("confidence")
            .and_then(|v| v.as_f64())
            .map(|c| c >= 0.3)
            .unwrap_or(true)
    });

    // Evict excess slots by confidence
    if slot_arr.len() > max_slots {
        slot_arr.sort_by(|a, b| {
            let ca = a.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let cb = b.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.0);
            cb.partial_cmp(&ca).unwrap_or(std::cmp::Ordering::Equal)
        });
        slot_arr.truncate(max_slots);
    }
}

/// Merge singleton updates (preferred_answer_style, preferred_language).
fn apply_singleton_update(
    profile: &mut serde_json::Value,
    key: &str,
    update: Option<&serde_json::Value>,
    today: &str,
) {
    let update = match update {
        Some(v) if !v.is_null() => v,
        _ => return,
    };
    let mut update = update.clone();
    if let Some(desc) = update.get("description").and_then(|v| v.as_str()) {
        update["description"] = serde_json::json!(truncate_text(desc, MAX_DESCRIPTION_LEN));
    }
    if let Some(evidence) = update.get_mut("evidence") {
        normalize_evidence(evidence);
    }
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

    let existing = profile
        .as_object_mut()
        .unwrap()
        .entry(key)
        .or_insert_with(|| serde_json::json!({}));

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

    // Evict if confidence too low
    if existing
        .get("confidence")
        .and_then(|v| v.as_f64())
        .map(|c| c < 0.3)
        .unwrap_or(false)
    {
        profile.as_object_mut().unwrap().remove(key);
    }
}

/// Merge session continuity hints (FIFO, max 3).
fn apply_hint_updates(
    profile: &mut serde_json::Value,
    hints: Option<&serde_json::Value>,
    today: &str,
) {
    let slot = profile
        .as_object_mut()
        .unwrap()
        .entry("session_continuity_hints")
        .or_insert_with(|| serde_json::json!([]));
    let slot_arr = slot.as_array_mut().unwrap();

    if let Some(hints) = hints.and_then(|v| v.as_array()).filter(|a| !a.is_empty()) {
        for hint in hints {
            let mut h = hint.clone();
            if let Some(priority) = h.get("priority").and_then(|v| v.as_str()) {
                if !VALID_HINT_PRIORITIES.contains(&priority) {
                    tracing::warn!(
                        priority,
                        "ignoring invalid session_continuity_hints priority"
                    );
                    continue;
                }
            }
            if let Some(text) = h.get("hint").and_then(|v| v.as_str()) {
                h["hint"] = serde_json::json!(truncate_text(text, MAX_DESCRIPTION_LEN));
            }
            h["created_at"] = serde_json::json!(today);
            slot_arr.push(h);
        }
    }

    // Always run expiration and FIFO eviction, even if no new hints arrived.
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(7))
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut profile = serde_json::json!({
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
        let merged = apply_profile_delta(profile, delta);
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
}
