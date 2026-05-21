use avrag_rag_core::context::SessionContext as RagSessionContext;
use avrag_storage_pg::{
    DocumentTaskSeed, NotificationCreateParams, PgAppRepository,
};
use common::{
    AppError, ChatMessage, ChatSession, DocumentStatus,
};
use ingestion::{
    AuditAction, IngestDocumentPayload, ReindexDocumentPayload, ReindexReason, build_ingest_task,
    build_reindex_task, task_audit,
};
use std::path::Path;
use uuid::Uuid;

use crate::lib_impl::*;

impl AppState {
    pub(crate) async fn list_ready_documents_for_chat(
        &self,
        notebook_id: &str,
        doc_scope: &[String],
    ) -> Vec<StoredDocument> {
        let state = self.inner.read().await;
        state
            .documents
            .values()
            .filter(|stored| stored.document.notebook_id == notebook_id)
            .filter(|stored| matches!(stored.document.status, DocumentStatus::Completed))
            .filter(|stored| doc_scope.is_empty() || doc_scope.contains(&stored.document.id))
            .cloned()
            .collect()
    }

    pub(crate) fn build_rag_session_context(
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

    pub(crate) async fn maybe_update_session_summary(
        &self,
        pg: &PgAppRepository,
        session: &ChatSession,
    ) -> bool {
        let Some(cm) = &self.chatmemory else {
            return false;
        };
        let Ok(session_uuid) =
            parse_uuid_or_app_error(&session.id, "session_not_found", "session not found")
        else {
            return false;
        };
        let Ok(messages) = pg.list_messages(&self.auth, session_uuid).await else {
            return false;
        };
        if messages.len() < 10 {
            return false;
        }

        let summary = self.build_session_summary(&messages).await;
        if summary.trim().is_empty() {
            return false;
        }

        cm.update_summary(&self.auth, session_uuid, &summary)
            .await
            .is_ok()
    }

    /// The "dream" layer: runs at most once per day per user.
    /// LLM proposes a semantic delta; runtime applies deterministic merge rules.
    pub(crate) async fn maybe_update_structured_profile(
        &self,
        pg: &PgAppRepository,
        _session: &ChatSession,
        session_summary: &str,
    ) -> bool {
        let Some(cm) = &self.chatmemory else {
            return false;
        };
        let Some(user_id) = self.auth.actor_id().map(|value| value.into_uuid()) else {
            return false;
        };

        let existing_profile = pg
            .get_user_profile(&self.auth, user_id)
            .await
            .ok()
            .flatten();

        let should_dream = match &existing_profile {
            Some(profile) => {
                let since_last =
                    chrono::Utc::now().signed_duration_since(profile.inferred_at);
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
            .infer_profile_delta(session_summary, &existing_structured)
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
        session_summary: &str,
        existing_profile: &serde_json::Value,
    ) -> serde_json::Value {
        const DREAM_SYSTEM_PROMPT: &str =
            include_str!("../../../../prompts/skills/user-profile-extraction/SKILL.md");
        let system_prompt = DREAM_SYSTEM_PROMPT.trim();
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let user_prompt = format!(
            "Today's date: {}\n\nExisting profile:\n{}\n\nRecent session summary:\n{}",
            today,
            serde_json::to_string_pretty(existing_profile).unwrap_or_else(|_| "{}".to_string()),
            session_summary.trim()
        );

        for (llm, temperature) in [
            (&self.memory_llm_client, self.memory_llm_temperature()),
            (&self.llm_client, self.agent_llm_temperature()),
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
                        self.record_llm_usage_if_available(
                            avrag_billing::usage_limit::BillableFeature::Summary,
                            "dream_delta",
                            &response.usage,
                            "inline",
                        )
                        .await;
                        return parse_structured_json_response(trimmed);
                    }
                }
        }
        serde_json::json!({})
    }

    pub(crate) async fn build_session_summary(&self, messages: &[ChatMessage]) -> String {
        const SESSION_SUMMARY_SYSTEM_PROMPT: &str = include_str!("../../../../prompts/skills/session-summary/SKILL.md");
        let summary_prompt = SESSION_SUMMARY_SYSTEM_PROMPT.trim();
        let prompt = messages
            .iter()
            .rev()
            .take(12)
            .rev()
            .map(|item| format!("{}: {}", item.role, item.content))
            .collect::<Vec<_>>()
            .join("\n");

        for (llm, temperature) in [
            (
                &self.memory_llm_client,
                self.memory_llm_temperature(),
            ),
            (
                &self.llm_client,
                self.agent_llm_temperature(),
            ),
        ] {
            if let Some(client) = llm
                && let Ok(response) = client
                    .complete(
                        &[
                            avrag_llm::ChatMessage::system(summary_prompt),
                            avrag_llm::ChatMessage::user(&prompt),
                        ],
                        temperature,
                    )
                    .await
                {
                    let trimmed = response.content.trim();
                    if !trimmed.is_empty() {
                        self.record_llm_usage_if_available(
                            avrag_billing::usage_limit::BillableFeature::Summary,
                            "session_summary",
                            &response.usage,
                            "inline",
                        )
                        .await;
                        return extract_summary_from_structured_response(trimmed);
                    }
                }
        }

        messages
            .iter()
            .rev()
            .take(6)
            .rev()
            .map(|item| format!("{}: {}", item.role, item.content))
            .collect::<Vec<_>>()
            .join(" | ")
            .chars()
            .take(320)
            .collect()
    }

    pub(crate) async fn emit_notification(
        &self,
        event_type: &str,
        title: &str,
        body: &str,
        data: serde_json::Value,
    ) -> Result<(), AppError> {
        let Some(pg) = &self.pg else {
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
        .await
        .map_err(map_pg_error)?;
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
        if let Some(ref qm) = self.quota_manager {
            let user_id = self
                .auth
                .actor_id()
                .map(|a| a.into_uuid())
                .unwrap_or_else(Uuid::nil);
            let org_id = self.auth.org_id().into_uuid();
            let ctx = avrag_billing::usage_limit::MeteringContext {
                user_id,
                org_id,
                feature,
                stage: stage.to_string(),
                session_id: None,
                document_id: None,
                request_id: self.auth.request_id().map(|s| s.to_string()),
                trace_id: None,
            };
            let _ = qm
                .rolling_service()
                .record_usage(
                    &ctx,
                    avrag_billing::usage_limit::UsageRecord {
                        provider: &non_empty_or_unknown(&usage.provider),
                        model: &non_empty_or_unknown(&usage.model),
                        prompt_tokens: usage.prompt_tokens,
                        completion_tokens: usage.completion_tokens,
                        total_tokens: usage.total_tokens,
                        usage_source: avrag_billing::usage_limit::UsageSource::Actual,
                    },
                )
                .await;
        }
        self.record_cost_event_if_available(
            crate::lib_impl::state_methods::CostEventRecord {
                event_name: analytics::CostEventName::LlmUsageMetered,
                feature: feature.as_str(),
                session_id: None,
                notebook_id: None,
                usage,
                source,
                metadata: serde_json::json!({
                    "stage": stage,
                    "feature": feature.as_str(),
                }),
            },
        )
        .await;
    }

    /// Get usage limit response for the current user.
    pub async fn get_user_usage_limit(
        &self,
    ) -> Result<avrag_billing::usage_limit::UsageLimitResponse, AppError> {
        let Some(ref qm) = self.quota_manager else {
            return Err(AppError::internal("quota service not configured"));
        };
        let user_id = self
            .auth
            .actor_id()
            .map(|a| a.into_uuid())
            .ok_or_else(|| AppError::internal("no authenticated user"))?;
        let org_id = self.auth.org_id().into_uuid();
        qm.rolling_service()
            .get_user_usage(org_id, user_id)
            .await
            .map_err(|e| AppError::internal(format!("failed to get usage limit: {}", e)))
    }

    /// Check if the current user has quota remaining.
    pub async fn check_user_quota(
        &self,
    ) -> Result<avrag_billing::usage_limit::QuotaCheckResult, AppError> {
        let Some(ref qm) = self.quota_manager else {
            return Err(AppError::internal("quota service not configured"));
        };
        let user_id = self
            .auth
            .actor_id()
            .map(|a| a.into_uuid())
            .unwrap_or_else(Uuid::nil);
        let org_id = self.auth.org_id().into_uuid();
        qm.rolling_service()
            .check_quota(org_id, user_id)
            .await
            .map_err(|e| AppError::internal(format!("usage limit check failed: {}", e)))
    }

    pub(crate) async fn ensure_metric_quota(&self, metric_type: &str, requested: i64) -> Result<(), AppError> {
        if requested <= 0 {
            return Ok(());
        }
        let Some(ref qm) = self.quota_manager else {
            return Ok(());
        };
        let user_uuid = self
            .auth
            .actor_id()
            .map(|v| v.into_uuid())
            .unwrap_or_else(Uuid::nil);
        let decision = qm
            .check_quota(
                self.auth.org_id().into_uuid(),
                user_uuid,
                metric_type,
                requested,
            )
            .await
            .map_err(map_anyhow_error)?;

        if decision.allowed {
            return Ok(());
        }

        let error_message = if let Some(reason) = &decision.reason {
            reason.clone()
        } else {
            format!("quota exceeded for {}", metric_type)
        };

        Err(AppError::rate_limited(
            "quota_exceeded",
            error_message,
            decision.retry_after_secs,
        ))
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
        let Some(pg) = &self.pg else {
            return Ok(());
        };
        pg.record_usage_event(&self.auth, metric_type, quantity, source)
            .await
            .map_err(map_pg_error)?;
        Ok(())
    }

    pub(crate) fn current_org_id(&self) -> String {
        self.auth.org_id().to_string()
    }

    pub(crate) fn current_user_id(&self) -> String {
        self.auth
            .actor_id()
            .map(|actor_id| actor_id.into_uuid().to_string())
            .unwrap_or_else(|| self.default_user_id())
    }

    pub fn signed_upload_url(
        &self,
        document_id: &str,
        object_path: &str,
        expires_at_unix: Option<u64>,
    ) -> Result<String, AppError> {
        let expires = expires_at_unix.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|value| value.as_secs())
                .unwrap_or_default()
                + self.object_storage_upload_expire_sec
        });
        let signature =
            sign_upload_payload(&upload_signing_secret(), document_id, object_path, expires)?;
        Ok(format!(
            "{}/uploads/{}?expires={}&signature={}",
            self.public_base_url, document_id, expires, signature
        ))
    }

    pub fn verify_upload_signature(
        &self,
        document_id: &str,
        object_path: &str,
        expires: u64,
        signature: &str,
    ) -> Result<(), AppError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_secs())
            .unwrap_or_default();
        if expires < now {
            return Err(AppError::validation(
                "upload_url_expired",
                "upload url expired",
            ));
        }
        let expected =
            sign_upload_payload(&upload_signing_secret(), document_id, object_path, expires)?;
        if expected != signature {
            return Err(AppError::validation(
                "invalid_upload_signature",
                "invalid upload signature",
            ));
        }
        Ok(())
    }

    pub(crate) fn object_root_path(&self) -> &Path {
        Path::new(&self.object_root)
    }

    pub(crate) async fn enqueue_ingest_task(&self, seed: DocumentTaskSeed) -> Result<(), AppError> {
        let Some(pg) = &self.pg else {
            return Ok(());
        };

        let task = build_ingest_task(
            seed.org_id.clone(),
            seed.notebook_id.clone(),
            seed.document_id.clone(),
            Some(self.current_user_id()),
            IngestDocumentPayload {
                source_uri: format!("object://{}", seed.object_path),
                object_path: seed.object_path.clone(),
                mime_type: seed.mime_type,
                filename: seed.filename,
                file_size: seed.file_size,
            },
        );
        let inserted = pg
            .enqueue_ingestion_task(&task)
            .await
            .map_err(map_pg_error)?;
        if inserted {
            pg.append_audit_record(&task_audit(
                &task,
                AuditAction::TaskEnqueued,
                serde_json::json!({
                    "kind": task.kind,
                    "document_id": task.document_id,
                    "object_path": match &task.payload {
                        ingestion::IngestionTaskPayload::IngestDocument(payload) => payload.object_path.clone(),
                        ingestion::IngestionTaskPayload::IngestUrl(payload) => payload.url.clone(),
                        ingestion::IngestionTaskPayload::ReindexDocument(_) => String::new(),
                    }
                }),
            ))
            .await
            .map_err(map_pg_error)?;
        }
        Ok(())
    }

    pub(crate) async fn enqueue_reindex_task(&self, seed: DocumentTaskSeed) -> Result<(), AppError> {
        let Some(pg) = &self.pg else {
            return Ok(());
        };

        let task = build_reindex_task(
            seed.org_id,
            seed.notebook_id,
            seed.document_id,
            Some(self.current_user_id()),
            ReindexDocumentPayload {
                reason: ReindexReason::Manual,
                requested_revision: (Uuid::new_v4().as_u128() & u32::MAX as u128) as u32,
            },
        );
        let inserted = pg
            .enqueue_ingestion_task(&task)
            .await
            .map_err(map_pg_error)?;
        if inserted {
            pg.append_audit_record(&task_audit(
                &task,
                AuditAction::TaskEnqueued,
                serde_json::json!({
                    "kind": task.kind,
                    "document_id": task.document_id,
                    "reason": "manual",
                }),
            ))
            .await
            .map_err(map_pg_error)?;
        }
        Ok(())
    }

    pub(crate) fn memory_session_visible(&self, state: &MemoryState, session: &ChatSession) -> bool {
        state
            .notebooks
            .get(&session.notebook_id)
            .map(|notebook| notebook.org_id == self.current_org_id())
            .unwrap_or(false)
    }

}

/// Parse a structured JSON session summary response and extract the prose summary field.
/// Falls back to the raw text if JSON parsing fails.
fn extract_summary_from_structured_response(text: &str) -> String {
    #[derive(Debug, serde::Deserialize)]
    struct StructuredSummary {
        #[serde(default)]
        summary: String,
    }

    let cleaned = text
        .trim()
        .strip_prefix("```json")
        .or_else(|| text.trim().strip_prefix("```"))
        .map(|s| s.trim())
        .and_then(|s| s.strip_suffix("```"))
        .map(|s| s.trim())
        .unwrap_or(text.trim());

    match serde_json::from_str::<StructuredSummary>(cleaned) {
        Ok(parsed) if !parsed.summary.trim().is_empty() => parsed.summary.trim().to_string(),
        _ => text.trim().to_string(),
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
fn apply_profile_delta(
    existing: serde_json::Value,
    delta: serde_json::Value,
) -> serde_json::Value {
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
                existing.push(c.clone());
            }
            // Keep last 10 conflicts
            if existing.len() > 10 {
                existing.truncate(10);
            }
        }
    }

    // ── global_summary ──
    if let Some(summary) = delta.get("global_summary").and_then(|v| v.as_str())
        && !summary.is_empty() {
            profile
                .as_object_mut()
                .unwrap()
                .insert("global_summary".to_string(), serde_json::json!(summary));
        }

    profile
}

/// Merge slot-array updates (expertise_domains, tool_preferences, constraints).
fn apply_slot_updates(
    profile: &mut serde_json::Value,
    key: &str,
    updates: Option<&serde_json::Value>,
    max_slots: usize,
    today: &str,
) {
    let updates = match updates.and_then(|v| v.as_array()) {
        Some(arr) if !arr.is_empty() => arr,
        _ => return,
    };

    let slots = profile
        .as_object_mut()
        .unwrap()
        .entry(key)
        .or_insert_with(|| serde_json::json!([]));
    let slot_arr = slots.as_array_mut().unwrap();

    for update in updates {
        let action = update.get("action").and_then(|v| v.as_str()).unwrap_or("none");
        let tag = update
            .get("tag")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if tag.is_empty() {
            continue;
        }

        let signal = update
            .get("confidence_signal")
            .and_then(|v| v.as_str())
            .unwrap_or("weak");
        let base_confidence = signal_to_confidence(signal);

        match action {
            "add" => {
                if slot_arr.iter().any(|s| s.get("tag") == Some(&serde_json::json!(tag))) {
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
                        && !desc.is_empty() {
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

    // Expire constraints
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
    let action = update.get("action").and_then(|v| v.as_str()).unwrap_or("none");
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
                && !desc.is_empty() {
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
                && !desc.is_empty() {
                    existing["description"] = serde_json::json!(desc);
                }
            if let Some(tag) = update.get("tag").and_then(|v| v.as_str())
                && !tag.is_empty() {
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
    let hints = match hints.and_then(|v| v.as_array()) {
        Some(arr) if !arr.is_empty() => arr,
        _ => return,
    };

    let slot = profile
        .as_object_mut()
        .unwrap()
        .entry("session_continuity_hints")
        .or_insert_with(|| serde_json::json!([]));
    let slot_arr = slot.as_array_mut().unwrap();

    for hint in hints {
        let mut h = hint.clone();
        h["created_at"] = serde_json::json!(today);
        slot_arr.push(h);
    }

    // Expire hints older than 7 days
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(7))
        .format("%Y-%m-%d")
        .to_string();
    slot_arr.retain(|h| {
        h.get("created_at")
            .and_then(|v| v.as_str())
            .map(|d| d >= cutoff.as_str())
            .unwrap_or(true)
    });

    // FIFO eviction
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
