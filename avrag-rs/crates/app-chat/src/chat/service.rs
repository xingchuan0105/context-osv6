use app_core::{parse_uuid_or_app_error, RetrievedContext};
use chrono::Utc;
use common::{AppError, now_rfc3339};
use contracts::chat::{ChatMessage, ChatRequest, ChatResponse, ModeDebug, TraceInfo};
use contracts::notebooks::{ChatSession, CreateChatSessionRequest};
use app_documents::{AuditAction, AuditRecord};
use tracing::info;
use uuid::Uuid;

use super::{ChatExecution, ChatPreflight, execute_chat_pipeline};
use crate::context::ChatContext;
use crate::{
    agent_icon, agent_name, build_answer, build_citations, build_degrade_trace, build_mode_debug,
    build_planner_output, build_sources, derive_profile_domains, derive_profile_topics,
    detect_preferred_style, estimate_token_count, merge_general_profile_custom_preferences,
    next_message_id,
};

impl ChatContext {
    #[tracing::instrument(skip(self, req), fields(agent_type = %req.agent_type, notebook_id = ?req.notebook_id))]
    pub async fn execute_chat_pipeline(
        &self,
        req: ChatRequest,
    ) -> Result<ChatResponse, AppError> {
        let effective_notebook_id = chat_notebook_id_for_request(self, &req);
        if self.storage.chat_persistence().is_some()
            && req.agent_type == "rag"
            && req.doc_scope.is_empty()
            && effective_notebook_id.is_none()
        {
            return Err(AppError::validation(
                "docscope_required",
                "Please select at least one document before using RAG.",
            ));
        }
        if req.agent_type == "rag" && !req.doc_scope.is_empty() {
            self.validate_rag_doc_scope(&req.doc_scope).await?;
        }
        execute_chat_pipeline(self.clone(), req).await
    }

    #[tracing::instrument(skip(self, req), fields(agent_type = %req.agent_type, notebook_id = ?req.notebook_id, trace_id = tracing::field::Empty))]
    pub(crate) async fn execute_chat_preflight(
        &self,
        req: &ChatRequest,
    ) -> Result<ChatPreflight, AppError> {
        let effective_notebook_id = chat_notebook_id_for_request(self, req);
        if req.source_type.as_deref() == Some("share") && self.auth.actor_id().is_none() {
            return Err(AppError::unauthorized(
                "Viewing shared content does not require sign-in, but asking questions does.",
            ));
        }
        let estimated_input_tokens = estimate_token_count(
            &std::iter::once(req.query.as_str())
                .chain(req.messages.iter().map(|item| item.content.as_str()))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        self.ensure_metric_quota("llm_input_tokens", estimated_input_tokens)
            .await?;
        self.ensure_metric_quota("llm_output_tokens", 1024).await?;

        let phase = self.billing.usage_limit_phase();
        let enforce_5h = phase == "5h_enforcement" || phase == "7d_enforcement";
        let enforce_7d = phase == "7d_enforcement";
        let quota = match self.check_user_quota().await {
            Ok(result) => result,
            Err(error) if enforce_5h || enforce_7d => return Err(error),
            Err(error) => {
                tracing::warn!(error = %error, "usage limit unavailable in shadow mode; continuing");
                avrag_billing::usage_limit::QuotaCheckResult::default()
            }
        };

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
                    "Usage limit exceeded for rolling 5h window: used {} / {} units. blocked_until={}.",
                    quota.used_5h,
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
                    "Usage limit exceeded for rolling 7d window: used {} / {} units. blocked_until={}.",
                    quota.used_7d,
                    quota.limit_7d,
                    blocked_until.unwrap_or_else(|| "unknown".to_string()),
                ),
                retry_after_secs,
            ));
        }

        let trace_id = Uuid::new_v4().to_string();
        tracing::Span::current().record("trace_id", &trace_id);
        let notebook_uuid = effective_notebook_id;
        if req.source_type.as_deref() == Some("share")
            && req.notebook_id.as_ref().is_some()
            && req.notebook_id.as_ref().and_then(|id| {
                parse_uuid_or_app_error(id, "invalid_notebook", "invalid notebook id").ok()
            }) != self.auth.notebook_id()
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

        let guard_scope = notebook_uuid
            .map(|id: Uuid| vec![id.to_string()])
            .unwrap_or_else(|| req.doc_scope.clone());

        info!(
            notebook_id = ?req.notebook_id,
            notebook_uuid = ?notebook_uuid,
            request_doc_scope = ?req.doc_scope,
            guard_scope = ?guard_scope,
            "chat preflight scope inputs"
        );

        let input_guard = self.orchestrator.guard_pipeline().check_input(
            &req.query,
            self.auth.org_id().into_uuid(),
            user_uuid,
            &guard_scope,
            notebook_uuid,
            Some(trace_id.clone()),
        );

        if !input_guard.passed {
            telemetry::prometheus::observe_guardrail_block(
                &input_guard.guard_type.to_string(),
                &input_guard.action.to_string(),
            );
            let audit_record = AuditRecord {
                audit_id: Uuid::new_v4().to_string(),
                org_id: self.auth.org_id().into_uuid().to_string(),
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
                    self.auth.org_id().into_uuid(),
                    user_uuid,
                    &guard_scope,
                    notebook_uuid,
                    Some(trace_id.clone()),
                );
                if !msg_guard.passed {
                    telemetry::prometheus::observe_guardrail_block(
                        &msg_guard.guard_type.to_string(),
                        &msg_guard.action.to_string(),
                    );
                    let audit_record = AuditRecord {
                        audit_id: Uuid::new_v4().to_string(),
                        org_id: self.auth.org_id().into_uuid().to_string(),
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
            notebook_uuid,
        })
    }

    #[tracing::instrument(skip(self, req), fields(agent_type = %req.agent_type, session_id = ?req.session_id))]
    pub(crate) async fn resolve_chat_session(
        &self,
        req: &ChatRequest,
    ) -> Result<ChatSession, AppError> {
        if req.source_type.as_deref() == Some("share") {
            let notebook_id = chat_notebook_id_for_request(self, req)
                .map(|value| value.to_string())
                .ok_or_else(|| {
                    AppError::validation("notebook_required", "notebook_id is required")
                })?;
            let session_id = req
                .session_id
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            let now = now_rfc3339();
            return Ok(ChatSession {
                id: session_id,
                notebook_id,
                title: None,
                agent_type: req.agent_type.clone(),
                summary: None,
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

        let notebook_id = chat_notebook_id_for_request(self, req)
            .map(|value| value.to_string())
            .ok_or_else(|| AppError::validation("notebook_required", "notebook_id is required"))?;
        if req.source_type.as_deref() == Some("share") {
            let now = now_rfc3339();
            return Ok(ChatSession {
                id: Uuid::new_v4().to_string(),
                notebook_id,
                title: None,
                agent_type: req.agent_type.clone(),
                summary: None,
                pinned: false,
                created_at: now.clone(),
                updated_at: now,
            });
        }
        self.create_session(CreateChatSessionRequest {
            notebook_id,
            title: None,
            agent_type: req.agent_type.clone(),
        })
        .await
    }
}

fn chat_notebook_id_for_request(state: &ChatContext, req: &ChatRequest) -> Option<Uuid> {
    req.notebook_id
        .as_ref()
        .and_then(|id| parse_uuid_or_app_error(id, "invalid_notebook", "invalid notebook id").ok())
        .or_else(|| {
            (req.source_type.as_deref() == Some("share"))
                .then(|| state.auth.notebook_id())
                .flatten()
        })
}

include!("service_modes.rs");
include!("service_postprocess.rs");
