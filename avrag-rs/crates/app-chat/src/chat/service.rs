use app_core::parse_uuid_or_app_error;
use app_documents::{AuditAction, AuditRecord};
use chrono::Utc;
use common::{AppError, now_rfc3339};
use contracts::chat::{ChatRequest, ChatResponse};
use contracts::workspaces::{ChatSession, ConversationScopeKind, CreateChatSessionRequest};
use tracing::info;
use uuid::Uuid;

use super::{ChatPreflight, PipelineLane, execute_pipeline};
use crate::context::ChatContext;
use crate::estimate_token_count;

impl ChatContext {
    #[tracing::instrument(skip(self, req), fields(agent_type = %req.agent_type, workspace_id = ?req.workspace_id))]
    pub async fn execute_chat_pipeline(
        &self,
        mut req: ChatRequest,
    ) -> Result<ChatResponse, AppError> {
        let effective_workspace_id = self.resolve_request_workspace(&mut req).await?;
        let state = self.with_owner_pays_auth(effective_workspace_id).await;
        // Single freeze (review P1-5): scope facts + history boundary are
        // captured once here, pre-preflight, and threaded through execution.
        // Enforcement consumes the frozen view; persist reuses the same set.
        let mut turn_scope_facts = state
            .turn_scope_facts(
                req.session_id.as_deref().and_then(|v| Uuid::parse_str(v).ok()),
                effective_workspace_id,
            )
            .await?;
        turn_scope_facts.history_boundary = match (
            req.session_id.as_deref().and_then(|v| Uuid::parse_str(v).ok()),
            state.chat_persistence(),
        ) {
            (Some(session_uuid), Some(persistence)) => persistence
                .list_messages(&state.auth, session_uuid)
                .await
                .map(|messages| messages.len())?,
            _ => 0,
        };
        enforce_agent_scope(&state, &mut req, &turn_scope_facts, effective_workspace_id).await?;
        execute_pipeline(state, req, PipelineLane::Agent, turn_scope_facts).await
    }

    /// The binding-derived scope facts of the current turn (W2d snapshot /
    /// evidence attribution share the same truth as scope enforcement).
    pub(crate) async fn turn_scope_facts(
        &self,
        session_id: Option<Uuid>,
        effective_workspace_id: Option<Uuid>,
    ) -> Result<TurnScopeFacts, AppError> {
        let mut facts = TurnScopeFacts::default();
        // No document store wired (bare memory test harnesses) → no file scope;
        // enforcement falls back to the pre-existing structural gates.
        if self.storage.document_store().is_none() {
            return Ok(facts);
        }
        if let Some(session_id) = session_id {
            if let Some(store) = self.storage.document_store() {
                for file in store.list_session_files(&self.auth, session_id).await? {
                    if file.status == "completed" {
                        facts.session_artifacts.push(SessionBindingVersion {
                            binding_id: file.binding_id,
                            artifact_id: file.document_id,
                            parse_version: file.parse_version,
                        });
                    }
                }
            }
        }
        if let Some(workspace_id) = effective_workspace_id {
            let raw = self
                .documents
                .completed_workspace_binding_versions(
                    &self.auth,
                    &self.storage,
                    &workspace_id.to_string(),
                )
                .await?;
            facts.workspace_binding_versions = raw
                .into_iter()
                .map(|version| serde_json::from_value(serde_json::to_value(&version).unwrap_or_default()))
                .collect::<Result<Vec<_>, _>>()
                .unwrap_or_default();
        }
        Ok(facts)
    }

    pub(crate) async fn resolve_request_workspace(
        &self,
        req: &mut ChatRequest,
    ) -> Result<Option<Uuid>, AppError> {
        if req.source_type.as_deref() != Some("share") {
            if let Some(session_id) = req.session_id.as_deref() {
                let requested_workspace = chat_workspace_id_for_request(self, req)?;
                let session = if let Some(session) = self.get_session(session_id).await {
                    session
                } else if let Some(workspace_id) = requested_workspace {
                    let workspace_state = self.with_owner_pays_auth(Some(workspace_id)).await;
                    let session =
                        workspace_state
                            .get_session(session_id)
                            .await
                            .ok_or_else(|| {
                                AppError::not_found("session_not_found", "session not found")
                            })?;
                    let session_workspace = session
                        .workspace_id
                        .as_deref()
                        .and_then(|value| Uuid::parse_str(value).ok());
                    if session_workspace != Some(workspace_id) {
                        return Err(AppError::not_found(
                            "session_not_found",
                            "session not found",
                        ));
                    }
                    session
                } else {
                    return Err(AppError::not_found(
                        "session_not_found",
                        "session not found",
                    ));
                };
                req.workspace_id = session.workspace_id;
            }
        }
        chat_workspace_id_for_request(self, req)
    }

    #[tracing::instrument(skip(self, req), fields(agent_type = %req.agent_type, workspace_id = ?req.workspace_id, trace_id = tracing::field::Empty))]
    pub(crate) async fn execute_chat_preflight(
        &self,
        req: &ChatRequest,
    ) -> Result<ChatPreflight, AppError> {
        let effective_workspace_id = chat_workspace_id_for_request(self, req)?;
        // ADR-0010: share chat may be anonymous when owner set workspace visibility
        // to `public`; auth middleware remaps `user_id` to the share owner.
        let is_share_chat = req.source_type.as_deref() == Some("share");
        // ADR-0010 §9: share input length fuse (anti sponge).
        if is_share_chat {
            let max_chars: usize = std::env::var("SHARE_CHAT_MAX_QUERY_CHARS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(4_000);
            if req.query.chars().count() > max_chars {
                return Err(AppError::validation(
                    "share_query_too_long",
                    format!("Share query exceeds max length of {max_chars} characters."),
                ));
            }
        }
        // ADR 0006 / 0010: rolling is protective, not the main product wall.
        // Skip hard rolling block when the payer account has wallet balance
        // (platform path) — BYOK skip is handled at usage observer later.
        // Monthly metric preflight remains soft.
        let estimated_input_tokens = estimate_token_count(
            &std::iter::once(req.query.as_str())
                .chain(req.messages.iter().map(|item| item.content.as_str()))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        if let Err(error) = self
            .ensure_metric_quota("llm_input_tokens", estimated_input_tokens)
            .await
        {
            tracing::warn!(
                error = %error,
                "monthly metric soft-limit exceeded; continuing (ADR 0006 soft limit)"
            );
        }
        if let Err(error) = self.ensure_metric_quota("llm_output_tokens", 1024).await {
            tracing::warn!(
                error = %error,
                "monthly metric soft-limit exceeded; continuing (ADR 0006 soft limit)"
            );
        }

        // ADR-0010 §1.1: no free ride on platform env keys without BYOK or balance.
        // Fail closed **before** LLM (not post-hoc wallet fail-open alone).
        // NOTE: usage_hold is placed only after ALL preflight gates pass (in pipeline),
        // so budget/hard-block rejections never leave a leaked hold.
        // Chat-first W3: consult the session's persisted model_role BEFORE the
        // spend gate — quick_chat sessions are exempt only via an active Quick
        // Chat BYOK config; generic llm BYOK never substitutes (and vice versa).
        // Without a session, the turn will CREATE one: personal /chat creates
        // quick_chat, workspace-bound creation creates agent (§4.2) — derive
        // the same default here.
        let byok_purpose = match req.session_id.as_deref() {
            Some(session_id) => {
                let model_role = self
                    .get_session(session_id)
                    .await
                    .map(|session| session.model_role)
                    .unwrap_or_else(|| "agent".to_string());
                if model_role == "quick_chat" {
                    app_core::ProviderSecretPurpose::QuickChat
                } else {
                    app_core::ProviderSecretPurpose::Llm
                }
            }
            None => {
                let workspace_bound = req.workspace_id.as_deref().is_some()
                    || self.auth.workspace_id().is_some();
                if workspace_bound {
                    app_core::ProviderSecretPurpose::Llm
                } else {
                    app_core::ProviderSecretPurpose::QuickChat
                }
            }
        };
        if let Err(error) = self
            .billing
            .ensure_payer_can_spend_for(&self.auth, byok_purpose)
            .await
        {
            if error.code() == "payer_funds_required" {
                // Throttled soft notify: emit for the billable owner (auth.user_id).
                let _ = self.emit_funds_required_notification().await;
            }
            return Err(error);
        }
        if is_share_chat {
            // ADR-0010 §4/§9: Owner daily fen fuse (platform proxy path only).
            self.billing
                .ensure_share_owner_daily_budget(&self.auth)
                .await?;
        }

        // Rolling windows: DoW protection for share + optional hard enforce kill-switch.
        // Private use with wallet/BYOK already passed ensure_payer_can_spend; rolling
        // hard-block only for share (owner burn) or USAGE_LIMIT_HARD_ENFORCE.
        let force_hard = matches!(
            std::env::var("USAGE_LIMIT_HARD_ENFORCE").as_deref(),
            Ok("1") | Ok("true") | Ok("yes")
        );
        let enforce_5h = is_share_chat || force_hard;
        let enforce_7d = is_share_chat || force_hard;
        let _phase = self.billing.usage_limit_phase();
        let quota = match self.check_user_quota().await {
            Ok(result) => result,
            Err(error) if enforce_5h || enforce_7d => return Err(error),
            Err(error) => {
                tracing::warn!(error = %error, "usage limit unavailable in shadow mode; continuing");
                avrag_billing::usage_limit::QuotaCheckResult::default()
            }
        };

        if (quota.soft_exceeded_5h || quota.soft_exceeded_7d)
            && !(quota.blocked_5h || quota.blocked_7d)
        {
            tracing::info!(
                used_5h = quota.used_5h,
                limit_5h = quota.limit_5h,
                used_7d = quota.used_7d,
                limit_7d = quota.limit_7d,
                hard_cap_5h = quota.hard_cap_5h,
                hard_cap_7d = quota.hard_cap_7d,
                "rolling soft limit exceeded; allowing request until hard cap (ADR 0006)"
            );
        }

        if quota.blocked_5h && enforce_5h {
            telemetry::prometheus::observe_usage_limit_block("5h");
            let blocked_until = quota.blocked_until_5h.map(|dt| dt.to_rfc3339());
            let retry_after_secs = quota
                .blocked_until_5h
                .and_then(|dt| (dt - Utc::now()).to_std().ok())
                .map(|d| d.as_secs().max(1))
                .unwrap_or(60);
            return Err(AppError::rate_limited(
                "usage_limit_exceeded",
                format!(
                    "Usage hard cap exceeded for rolling 5h window: used {} / hard_cap {} (plan limit {}). blocked_until={}.",
                    quota.used_5h,
                    quota.hard_cap_5h,
                    quota.limit_5h,
                    blocked_until.unwrap_or_else(|| "unknown".to_string()),
                ),
                retry_after_secs,
            ));
        }
        if quota.blocked_7d && enforce_7d {
            telemetry::prometheus::observe_usage_limit_block("7d");
            let blocked_until = quota.blocked_until_7d.map(|dt| dt.to_rfc3339());
            let retry_after_secs = quota
                .blocked_until_7d
                .and_then(|dt| (dt - Utc::now()).to_std().ok())
                .map(|d| d.as_secs().max(1))
                .unwrap_or(60);
            return Err(AppError::rate_limited(
                "usage_limit_exceeded",
                format!(
                    "Usage hard cap exceeded for rolling 7d window: used {} / hard_cap {} (plan limit {}). blocked_until={}.",
                    quota.used_7d,
                    quota.hard_cap_7d,
                    quota.limit_7d,
                    blocked_until.unwrap_or_else(|| "unknown".to_string()),
                ),
                retry_after_secs,
            ));
        }

        let trace_id = Uuid::new_v4().to_string();
        tracing::Span::current().record("trace_id", &trace_id);
        let workspace_uuid = effective_workspace_id;
        if req.source_type.as_deref() == Some("share")
            && req.workspace_id.as_ref().is_some()
            && effective_workspace_id != self.auth.workspace_id()
        {
            return Err(AppError::validation(
                "invalid_share_scope",
                "share token does not match notebook scope",
            ));
        }
        let user_uuid = self
            .auth
            .actor_id()
            .map(|actor| actor.into_uuid())
            .unwrap_or_else(Uuid::nil);

        let guard_scope = workspace_uuid
            .map(|id: Uuid| vec![id.to_string()])
            .unwrap_or_else(|| req.doc_scope.clone());

        info!(
            workspace_id = ?req.workspace_id,
            workspace_uuid = ?workspace_uuid,
            request_doc_scope = ?req.doc_scope,
            guard_scope = ?guard_scope,
            "chat preflight scope inputs"
        );

        let input_guard = self.orchestrator.guard_pipeline().check_input(
            &req.query,
            self.auth.user_id().into_uuid(),
            user_uuid,
            &guard_scope,
            workspace_uuid,
            Some(trace_id.clone()),
        );

        if !input_guard.passed {
            telemetry::prometheus::observe_guardrail_block(
                &input_guard.guard_type.to_string(),
                &input_guard.action.to_string(),
            );
            let audit_record = AuditRecord {
                audit_id: Uuid::new_v4().to_string(),
                owner_user_id: self.auth.user_id().into_uuid().to_string(),
                actor_id: Some(user_uuid.to_string()),
                action: AuditAction::InputGuardBlock,
                resource_type: "chat".to_string(),
                resource_id: String::new(),
                payload: serde_json::json!({
                    "guard_type": input_guard.guard_type,
                    "risk_level": input_guard.risk_level.to_string(),
                    "action": input_guard.action.to_string(),
                    "reason": input_guard.reason,
                    "trace_id": trace_id,
                }),
                created_at: now_rfc3339(),
            };
            if let Some(pg) = self.storage.chat_persistence() {
                let _ = pg.append_audit_record(&audit_record).await;
            }
            return Err(AppError::validation(
                "input_guard_blocked",
                format!("Query blocked by guard: {}", input_guard.reason),
            ));
        }

        // R1: Check history messages for prompt injection bypass attempts.
        for msg in &req.messages {
            if msg.role == "user" {
                let msg_guard = self.orchestrator.guard_pipeline().check_input(
                    &msg.content,
                    self.auth.user_id().into_uuid(),
                    user_uuid,
                    &guard_scope,
                    workspace_uuid,
                    Some(trace_id.clone()),
                );
                if !msg_guard.passed {
                    telemetry::prometheus::observe_guardrail_block(
                        &msg_guard.guard_type.to_string(),
                        &msg_guard.action.to_string(),
                    );
                    let audit_record = AuditRecord {
                        audit_id: Uuid::new_v4().to_string(),
                        owner_user_id: self.auth.user_id().into_uuid().to_string(),
                        actor_id: Some(user_uuid.to_string()),
                        action: AuditAction::InputGuardBlock,
                        resource_type: "chat".to_string(),
                        resource_id: String::new(),
                        payload: serde_json::json!({
                            "guard_type": msg_guard.guard_type,
                            "risk_level": msg_guard.risk_level.to_string(),
                            "action": msg_guard.action.to_string(),
                            "reason": msg_guard.reason,
                            "trace_id": trace_id,
                            "source": "history_message",
                        }),
                        created_at: now_rfc3339(),
                    };
                    if let Some(pg) = self.storage.chat_persistence() {
                        let _ = pg.append_audit_record(&audit_record).await;
                    }
                    return Err(AppError::validation(
                        "input_guard_blocked",
                        format!("Message blocked by guard: {}", msg_guard.reason),
                    ));
                }
            }
        }

        Ok(ChatPreflight {
            trace_id,
            user_uuid,
            workspace_uuid,
            byok_purpose,
            // Hold placed in pipeline after cache miss + all gates (not here).
            usage_hold_id: None,
            usage_hold_fen: None,
            estimated_input_tokens,
        })
    }

    #[tracing::instrument(skip(self, req), fields(agent_type = %req.agent_type, session_id = ?req.session_id))]
    pub(crate) async fn resolve_chat_session(
        &self,
        req: &ChatRequest,
    ) -> Result<ChatSession, AppError> {
        if req.source_type.as_deref() == Some("share") {
            let workspace_id = chat_workspace_id_for_request(self, req)?
                .map(|value| value.to_string())
                .ok_or_else(|| {
                    AppError::validation("workspace_required", "workspace_id is required")
                })?;
            let session_id = req
                .session_id
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            let now = now_rfc3339();
            return Ok(ChatSession {
                id: session_id,
                workspace_id: Some(workspace_id),
                scope_kind: ConversationScopeKind::Workspace,
                workspace_name: None,
                owner_user_id: self.current_owner_user_id(),
                title: None,
                agent_type: req.agent_type.clone(),
                model_role: "agent".to_string(),
                pinned: false,
                created_at: now.clone(),
                updated_at: now,
            });
        }

        if let Some(session_id) = req.session_id.clone() {
            return self
                .get_session(&session_id)
                .await
                .ok_or_else(|| AppError::not_found("session_not_found", "session not found"));
        }

        let workspace_id = chat_workspace_id_for_request(self, req)?;
        self.create_session(CreateChatSessionRequest {
            workspace_id: workspace_id.map(|value| value.to_string()),
            title: None,
            agent_type: Some(req.agent_type.clone()),
        })
        .await
    }
}

fn chat_workspace_id_for_request(
    state: &ChatContext,
    req: &ChatRequest,
) -> Result<Option<Uuid>, AppError> {
    let requested_workspace = req
        .workspace_id
        .as_deref()
        .map(|id| {
            parse_uuid_or_app_error(
                id,
                "invalid_workspace_id",
                "workspace_id must be a valid UUID",
            )
        })
        .transpose()?;
    Ok(requested_workspace.or_else(|| {
        (req.source_type.as_deref() == Some("share"))
            .then(|| state.auth.workspace_id())
            .flatten()
    }))
}

/// Binding-derived scope facts of one turn: which ready artifacts are visible
/// through the conversation binding vs a workspace binding.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct WorkspaceBindingVersion {
    pub binding_id: String,
    pub artifact_id: String,
    pub parse_version: Option<String>,
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct TurnScopeFacts {
    /// Binding + artifact + parse version facts of conversation-bound files.
    pub session_artifacts: Vec<SessionBindingVersion>,
    /// Binding + artifact + parse version facts of workspace-bound files.
    pub workspace_binding_versions: Vec<WorkspaceBindingVersion>,
    /// Pre-execution count of persisted messages for this conversation
    /// (the history boundary this turn was built on). Frozen before the run.
    pub history_boundary: usize,
}

impl TurnScopeFacts {
    pub(crate) fn allowed_ids(&self) -> std::collections::HashSet<String> {
        let mut ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        ids.extend(self.session_artifacts.iter().map(|b| b.artifact_id.clone()));
        ids.extend(self.workspace_binding_versions.iter().map(|b| b.artifact_id.clone()));
        ids
    }

    /// All visible scopes for an artifact — dual bindings keep BOTH provenances
    /// (review round-3: session precedence alone loses the workspace view).
    pub(crate) fn scopes_of(&self, artifact_id: &str) -> Vec<&'static str> {
        let mut scopes = Vec::new();
        if self.session_artifacts.iter().any(|b| b.artifact_id == artifact_id) {
            scopes.push("session");
        }
        if self
            .workspace_binding_versions
            .iter()
            .any(|b| b.artifact_id == artifact_id)
        {
            scopes.push("workspace");
        }
        scopes
    }
}

/// One conversation binding's version facts (design §4.4 binding+version).
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct SessionBindingVersion {
    pub binding_id: String,
    pub artifact_id: String,
    pub parse_version: Option<String>,
}

/// Authoritative RAG-turn predicate: `capabilities[]` wins over the derived
/// `agent_type` label (which may be "chat", "rag", "search" or "rag+search").
pub(crate) fn is_rag_turn(req: &ChatRequest) -> bool {
    // Canonical resolver (review round-3 P1): when `capabilities` is present it
    // is authoritative — even empty — and the legacy agent_type label is
    // ignored, exactly like the execution router.
    matches!(
        crate::resolve_capabilities(req.capabilities.as_deref(), &req.agent_type),
        Ok(set) if set.rag
    )
}

/// Server-side ContextScope enforcement shared by BOTH chat entries — the
/// non-streaming pipeline and the SSE streaming path (review P0-2: session
/// files must ride the main streaming path too). Injects ready session
/// artifacts into RAG turns, then applies the structural gates.
pub(crate) async fn enforce_agent_scope(
    state: &ChatContext,
    req: &mut ChatRequest,
    facts: &TurnScopeFacts,
    effective_workspace_id: Option<Uuid>,
) -> Result<(), AppError> {
    let client_scope_was_empty = req.doc_scope.is_empty();
    if !req.doc_scope.is_empty() {
        let allowed = facts.allowed_ids();
        req.doc_scope.retain(|id| allowed.contains(id));
    }
    // Chat-first closure: ready session artifacts join RAG turns by default.
    // The scope is derived server-side from typed bindings — the client can
    // narrow workspace selections but cannot add or hide session files.
    // `capabilities[]` is the authoritative routing fact (agent_type is a
    // derived label, e.g. "rag+search").
    if is_rag_turn(req) {
        for binding in &facts.session_artifacts {
            if !req.doc_scope.contains(&binding.artifact_id) {
                req.doc_scope.push(binding.artifact_id.clone());
            }
        }
    }
    if state.storage.chat_persistence().is_some()
        && is_rag_turn(req)
        && req.doc_scope.is_empty()
        && effective_workspace_id.is_none()
    {
        return Err(AppError::validation(
            "docscope_required",
            "Please select at least one document before using RAG.",
        ));
    }
    if is_rag_turn(req) && !client_scope_was_empty && req.doc_scope.is_empty() {
        // The client selected documents, but none survived the binding-derived
        // allowed set: fail closed instead of silently widening the scope.
        return Err(AppError::validation(
            "invalid_doc_scope",
            "No selected document is available in this conversation's context scope.",
        ));
    }
    if is_rag_turn(req) && !req.doc_scope.is_empty() {
        state.validate_rag_doc_scope(&req.doc_scope).await?;
    }
    Ok(())
}
