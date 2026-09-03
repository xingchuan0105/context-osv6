use app_core::parse_uuid_or_app_error;
use app_documents::{AuditAction, AuditRecord};
use common::{AppError, now_rfc3339};
use contracts::chat::ChatRequest;
use contracts::workspaces::ChatSession;
use tracing::info;
use uuid::Uuid;

use super::ChatExecution;
use crate::context::ChatContext;
use crate::estimate_token_count;

fn session_workspace_uuid(session: &ChatSession) -> Option<Uuid> {
    session
        .workspace_id
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok())
}

impl ChatContext {
    pub(crate) async fn apply_output_guard_to_execution(
        &self,
        session: &ChatSession,
        execution: &mut ChatExecution,
        trace_id: &str,
        user_uuid: Uuid,
        chat_persistence: Option<&dyn app_core::ChatPersistencePort>,
    ) -> Result<(), AppError> {
        if !execution.apply_output_guard {
            return Ok(());
        }

        let (sanitized_answer, guard_report) = self
            .orchestrator
            .guard_pipeline()
            .check_output(&execution.response.answer, Some(trace_id.to_string()));

        execution.response.answer = sanitized_answer;
        for item in &guard_report.degrade_trace {
            execution.response.degrade_trace.push(item.clone());
        }
        execution.response.guard_report = Some(guard_report.clone());

        for result in &guard_report.output_results {
            if !result.passed || result.action == contracts::chat::GuardAction::Redact {
                telemetry::prometheus::observe_guardrail_block(
                    &result.guard_type.to_string(),
                    &result.action.to_string(),
                );
                let audit_action = match result.action {
                    contracts::chat::GuardAction::Block => AuditAction::OutputGuardBlock,
                    contracts::chat::GuardAction::Redact => AuditAction::OutputGuardRedact,
                    contracts::chat::GuardAction::Flag => AuditAction::OutputGuardFlag,
                    _ => continue,
                };
                let audit_record = AuditRecord {
                    audit_id: Uuid::new_v4().to_string(),
                    owner_user_id: self.auth.user_id().into_uuid().to_string(),
                    actor_id: Some(user_uuid.to_string()),
                    action: audit_action,
                    resource_type: "chat".to_string(),
                    resource_id: session.id.clone(),
                    payload: serde_json::json!({
                        "guard_type": result.guard_type,
                        "risk_level": result.risk_level.to_string(),
                        "reason": result.reason,
                        "trace_id": trace_id,
                    }),
                    created_at: now_rfc3339(),
                };
                if let Some(chat_persistence) = chat_persistence {
                    let _ = chat_persistence.append_audit_record(&audit_record).await;
                }
            }
        }

        Ok(())
    }

    pub(crate) async fn persist_chat_execution(
        &self,
        req: &ChatRequest,
        session: &ChatSession,
        execution: &mut ChatExecution,
        chat_persistence: &dyn app_core::ChatPersistencePort,
    ) -> Result<(), AppError> {
        let session_uuid =
            parse_uuid_or_app_error(&session.id, "session_not_found", "session not found")?;
        info!(
            session_id = %session.id,
            answer_blocks_count = execution.response.answer_blocks.len(),
            answer_blocks = ?execution.response.answer_blocks,
            "persisting assistant answer blocks"
        );
        let tool_results: Vec<contracts::ToolResult> = execution
            .response
            .tool_results
            .iter()
            .map(|r| contracts::ToolResult::from(r.clone()))
            .collect();
        // ADR-0010: server-side query normalization removed; no per-turn
        // resolved_query or query_resolution metadata is persisted.
        // Persist capabilities for UI replay (prefer resolved assistant meta; fall back to request).
        let resolved_capabilities: Option<serde_json::Value> = execution
            .assistant_turn_metadata
            .as_ref()
            .and_then(|m| m.get("capabilities").cloned())
            .or_else(|| {
                req.capabilities
                    .as_ref()
                    .map(|c| serde_json::to_value(c).ok())
                    .flatten()
            });
        let web_enabled = execution.mode == "search"
            || resolved_capabilities.as_ref().is_some_and(|c| {
                c.as_array().is_some_and(|items| {
                    items.iter().any(|v| v.as_str() == Some("search"))
                })
            });
        // W2d: freeze what this turn was allowed to use (design §4.4). The
        // facts were captured BEFORE execution (run_pipeline_inner) and are
        // attached to this execution — never re-queried, never silently empty.
        let scope_facts = execution.turn_scope_facts.clone();
        let snapshot_id = uuid::Uuid::new_v4().to_string();
        let context_snapshot = serde_json::json!({
            "snapshot_id": snapshot_id,
            "workspace_id_at_send": session.workspace_id,
            "conversation_history_boundary": {
                "persisted_messages": scope_facts.history_boundary,
            },
            "session_binding_versions": scope_facts.session_artifacts,
            "workspace_binding_artifacts": scope_facts.workspace_artifacts,
            "web_enabled": web_enabled,
            "thinking_enabled": null,
            "model_role": session.model_role,
            "credential_source": execution.response.credential_source,
            "effective_provider": execution
                .response
                .usage
                .as_ref()
                .and_then(|usage| usage.provider.clone()),
            "effective_model": execution
                .response
                .usage
                .as_ref()
                .and_then(|usage| usage.model.clone()),
            "created_at": now_rfc3339(),
        });
        let user_turn_metadata: Option<serde_json::Value> = {
            let mut meta = serde_json::Map::new();
            if let Some(caps) = resolved_capabilities {
                meta.insert("capabilities".to_string(), caps);
            }
            meta.insert("context_snapshot".to_string(), context_snapshot);
            Some(serde_json::Value::Object(meta))
        };
        // §10.4: completion surfaces the frozen snapshot id (persist runs
        // before the SSE done event, so live payloads carry it too).
        execution.response.turn_context_snapshot_id = Some(snapshot_id);
        // W2d: what this turn actually used — scope-tagged citations, written
        // once with the assistant row. Tagging here also flows into the SSE
        // done payload (persist runs before Done).
        let mut evidence_segments: Vec<serde_json::Value> = Vec::new();
        for citation in &mut execution.response.citations {
            citation.source_scope = scope_facts.scope_of(&citation.doc_id).map(str::to_string);
            if let Some(scope) = citation.source_scope.clone() {
                evidence_segments.push(serde_json::json!({
                    "channel": "rag",
                    "source_scope": scope,
                    "artifact_id": citation.doc_id,
                    "chunk_id": citation.chunk_id,
                    "page": citation.page,
                    "asset_id": citation.asset_id,
                    "parse_version": citation.parse_run_id,
                }));
            }
        }
        let user_resolved_query: Option<&str> = None;
        let mut assistant_turn_metadata = execution.assistant_turn_metadata.clone();
        {
            let meta = assistant_turn_metadata
                .get_or_insert_with(|| serde_json::json!({}));
            if let Some(obj) = meta.as_object_mut() {
                obj.insert(
                    "turn_evidence".to_string(),
                    serde_json::json!({ "segments": evidence_segments }),
                );
            }
        }
        execution.assistant_turn_metadata = assistant_turn_metadata.clone();
        // Prefer derived capability label (chat|rag|search|rag+search) over raw request.agent_type.
        let persist_agent_type = execution.response.agent_type.as_str();
        let assistant_message_id = chat_persistence
            .append_chat_turn(
                &self.auth,
                session_uuid,
                app_core::AppendChatTurn {
                    user_content: req.query.trim(),
                    assistant_content: &execution.response.answer,
                    assistant_answer_blocks: &execution.response.answer_blocks,
                    agent_type: persist_agent_type,
                    citations: &execution.response.citations,
                    tool_results: &tool_results,
                    user_turn_metadata,
                    user_resolved_query,
                    assistant_turn_metadata,
                },
            )
            .await?;
        execution.response.message_id = Some(assistant_message_id);

        let _ = self
            .remember_explicit_agent_preference(req.query.trim())
            .await;

        // Best-effort profile enrichment (returns bool; never fails the turn).
        let _ = self
            .maybe_update_profiles(chat_persistence, session_uuid, execution)
            .await;

        let event_name = if req.source_type.as_deref() == Some("share") {
            analytics::ProductEventName::SharedKbChatCompleted
        } else if execution.mode == "search" {
            analytics::ProductEventName::SearchCompleted
        } else {
            analytics::ProductEventName::ChatCompleted
        };
        let result = if execution.response.degrade_trace.is_empty() {
            analytics::ResultTag::Success
        } else {
            analytics::ResultTag::Degraded
        };
        let mut metadata = serde_json::json!({
            "agent_type": req.agent_type,
            "mode": execution.mode,
            "message_id": execution.response.message_id,
            "citation_count": execution.response.citations.len(),
            "degrade_count": execution.response.degrade_trace.len(),
        });
        if let Some(debug_metadata) = execution.debug_metadata.as_ref()
            && let (Some(metadata), Some(debug_metadata)) =
                (metadata.as_object_mut(), debug_metadata.as_object())
        {
            for (key, value) in debug_metadata {
                metadata.insert(key.clone(), value.clone());
            }
        }

        self.record_product_event_if_available(
            event_name,
            if req.source_type.as_deref() == Some("share") {
                analytics::Surface::SharedKb
            } else if session.workspace_id.is_some() {
                analytics::Surface::Workspace
            } else {
                analytics::Surface::Chat
            },
            result,
            Uuid::parse_str(&session.id).ok(),
            session
                .workspace_id
                .as_deref()
                .and_then(|value| Uuid::parse_str(value).ok()),
            metadata,
        )
        .await;

        Ok(())
    }

    pub(crate) async fn record_usage_for_execution(
        &self,
        execution: &ChatExecution,
    ) -> Result<(), AppError> {
        let scope = format!("{}_chat", execution.mode);

        // Monthly plan counters (`usage_events`) prefer **actual** provider tokens when
        // the agent reported them. Estimated counts remain only as a fallback so offline
        // / no-LLM paths still move the meter. Per-call `llm_usage_events` are written
        // solely by exit-metering (`UsageObserver`) — never re-insert them here.
        let (input_units, output_units) = if let Some(ref llm_usage) = execution.llm_usage {
            (
                i64::from(llm_usage.prompt_tokens),
                i64::from(llm_usage.completion_tokens),
            )
        } else {
            (
                estimate_token_count(&execution.input_usage_text),
                estimate_token_count(&execution.response.answer),
            )
        };
        let _ = self
            .record_usage("llm_input_tokens", input_units, &scope)
            .await;
        let _ = self
            .record_usage("llm_output_tokens", output_units, &scope)
            .await;

        if let Some(ref llm_usage) = execution.llm_usage {
            let feature = match execution.mode.as_str() {
                "chat" => "chat",
                "search" => "search",
                "rag" => "answer",
                _ => "chat",
            };
            if matches!(execution.mode.as_str(), "search" | "rag") {
                telemetry::prometheus::observe_retrieval_request(&execution.mode, "final");
                if execution.response.citations.is_empty() {
                    telemetry::prometheus::observe_retrieval_zero_result(&execution.mode);
                }
            }
            let mut cost_metadata = serde_json::json!({
                "mode": execution.mode,
                "degrade_count": execution.response.degrade_trace.len(),
            });
            if let Some(debug_metadata) = execution.debug_metadata.as_ref()
                && let Some(tool_telemetry) = debug_metadata.get("tool_telemetry")
            {
                cost_metadata["tool_telemetry"] = tool_telemetry.clone();
            }
            self.record_cost_event_if_available(app_billing::CostEventRecord {
                event_name: analytics::CostEventName::LlmUsageMetered,
                feature,
                session_id: Uuid::parse_str(&execution.response.session_id).ok(),
                workspace_id: None,
                usage: llm_usage,
                source: "pipeline",
                metadata: cost_metadata,
            })
            .await;
        }

        Ok(())
    }
}
