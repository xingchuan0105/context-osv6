impl AppState {
    pub(crate) async fn apply_output_guard_to_execution(
        &self,
        session: &ChatSession,
        execution: &mut ChatGraphExecution,
        trace_id: &str,
        user_uuid: Uuid,
        pg: Option<&PgAppRepository>,
    ) -> Result<(), AppError> {
        if !execution.apply_output_guard {
            return Ok(());
        }

        let citation_chunk_ids: Vec<Uuid> = execution
            .response
            .citations
            .iter()
            .filter_map(|citation| {
                citation
                    .chunk_id
                    .as_ref()
                    .and_then(|chunk_id| Uuid::parse_str(chunk_id).ok())
            })
            .collect();
        let (sanitized_answer, guard_report) = self.guard_pipeline.check_output(
            &execution.response.answer,
            &execution.response.citations,
            &citation_chunk_ids,
            Some(trace_id.to_string()),
        );

        execution.response.answer = sanitized_answer;
        for item in &guard_report.degrade_trace {
            execution.response.degrade_trace.push(item.clone());
        }
        execution.response.guard_report = Some(guard_report.clone());

        for result in &guard_report.output_results {
            if !result.passed || result.action == common::GuardAction::Redact {
                telemetry::prometheus::observe_guardrail_block(
                    &result.guard_type.to_string(),
                    &result.action.to_string(),
                );
                let audit_action = match result.action {
                    common::GuardAction::Block => AuditAction::OutputGuardBlock,
                    common::GuardAction::Redact => AuditAction::OutputGuardRedact,
                    common::GuardAction::Flag => AuditAction::OutputGuardFlag,
                    _ => continue,
                };
                let audit_record = AuditRecord {
                    audit_id: Uuid::new_v4().to_string(),
                    org_id: self.auth.org_id().into_uuid().to_string(),
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
                if let Some(pg) = pg {
                    let _ = pg.append_audit_record(&audit_record).await;
                }
            }
        }

        Ok(())
    }

    pub(crate) async fn persist_chat_execution(
        &self,
        req: &ChatRequest,
        session: &ChatSession,
        execution: &mut ChatGraphExecution,
        pg: &PgAppRepository,
    ) -> Result<(), AppError> {
        let session_uuid =
            parse_uuid_or_app_error(&session.id, "session_not_found", "session not found")?;
        info!(
            session_id = %session.id,
            answer_blocks_count = execution.response.answer_blocks.len(),
            answer_blocks = ?execution.response.answer_blocks,
            "persisting assistant answer blocks"
        );
        let assistant_message_id = pg
            .append_chat_turn(
                &self.auth,
                session_uuid,
                req.query.trim(),
                &execution.response.answer,
                &execution.response.answer_blocks,
                &req.agent_type,
                &execution.response.citations,
            )
            .await
            .map_err(map_pg_error)?;
        execution.response.message_id = Some(assistant_message_id);

        let summary_updated = self.maybe_update_session_summary(pg, session).await;

        if execution.mode == "general"
            && let Some(ref cm) = self.chatmemory
            && let Ok(messages) = pg.list_messages(&self.auth, session_uuid).await
        {
            let _ = cm
                .update_user_profile(
                    &self.auth,
                    derive_profile_domains(&messages, req.query.trim()),
                    detect_preferred_style(req.query.trim()),
                    derive_profile_topics(&messages, req.query.trim()),
                    serde_json::json!({
                        "last_general_query": req.query.trim(),
                        "refined_query": execution.input_usage_text,
                    }),
                    "general-v1",
                )
                .await;
            let _ = cm
                .update_working_memory(
                    &self.auth,
                    session_uuid,
                    "working_memory",
                    infer_current_topic(req.query.trim()),
                    extract_pending_questions(req.query.trim()),
                    extract_gathered_facts(&execution.response.answer),
                    if execution.response.degrade_trace.is_empty() {
                        0.82
                    } else {
                        0.35
                    },
                    vec![req.query.trim().to_string()],
                )
                .await;

            if summary_updated
                && let Some(mode_debug) = execution.response.mode_debug.as_mut()
                && let Some(general) = mode_debug.general.as_mut()
            {
                general.insert("summary_updated".to_string(), serde_json::json!(true));
            }
        }

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
        self.record_product_event_if_available(
            event_name,
            if req.source_type.as_deref() == Some("share") {
                analytics::Surface::SharedKb
            } else {
                analytics::Surface::Workspace
            },
            result,
            Uuid::parse_str(&session.id).ok(),
            Uuid::parse_str(&session.notebook_id).ok(),
            serde_json::json!({
                "agent_type": req.agent_type,
                "mode": execution.mode,
                "message_id": execution.response.message_id,
                "citation_count": execution.response.citations.len(),
                "degrade_count": execution.response.degrade_trace.len(),
            }),
        )
        .await;

        Ok(())
    }

    pub(crate) async fn record_usage_for_execution(
        &self,
        execution: &ChatGraphExecution,
    ) -> Result<(), AppError> {
        let scope = format!("{}_chat", execution.mode);
        let _ = self
            .record_usage(
                "llm_input_tokens",
                estimate_token_count(&execution.input_usage_text),
                &scope,
            )
            .await;
        let _ = self
            .record_usage(
                "llm_output_tokens",
                estimate_token_count(&execution.response.answer),
                &scope,
            )
            .await;

        if let Some(ref usage_svc) = self.usage_limit {
            if let Some(ref llm_usage) = execution.llm_usage {
                let feature = match execution.mode.as_str() {
                    "general" => avrag_usage_limit::BillableFeature::Chat,
                    "search" => avrag_usage_limit::BillableFeature::Search,
                    "rag" => avrag_usage_limit::BillableFeature::Answer,
                    _ => avrag_usage_limit::BillableFeature::Chat,
                };
                let user_id = self
                    .auth
                    .actor_id()
                    .map(|a| a.into_uuid())
                    .unwrap_or_else(Uuid::nil);
                let org_id = self.auth.org_id().into_uuid();
                let ctx = avrag_usage_limit::MeteringContext {
                    user_id,
                    org_id,
                    feature,
                    stage: format!("{}_chat", execution.mode),
                    session_id: None,
                    document_id: None,
                    request_id: self.auth.request_id().map(|s| s.to_string()),
                    trace_id: None,
                };
                let _ = usage_svc
                    .record_usage(
                        &ctx,
                        if llm_usage.provider.trim().is_empty() {
                            "unknown"
                        } else {
                            llm_usage.provider.as_str()
                        },
                        if llm_usage.model.trim().is_empty() {
                            "unknown"
                        } else {
                            llm_usage.model.as_str()
                        },
                        llm_usage.prompt_tokens,
                        llm_usage.completion_tokens,
                        llm_usage.total_tokens,
                        avrag_usage_limit::UsageSource::Actual,
                    )
                    .await;
            }
        }

        if let Some(ref llm_usage) = execution.llm_usage {
            let feature = match execution.mode.as_str() {
                "general" => "chat",
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
            self.record_cost_event_if_available(
                analytics::CostEventName::LlmUsageMetered,
                feature,
                Uuid::parse_str(&execution.response.session_id).ok(),
                None,
                llm_usage,
                "graphflow",
                serde_json::json!({
                    "mode": execution.mode,
                    "degrade_count": execution.response.degrade_trace.len(),
                }),
            )
            .await;
        }

        Ok(())
    }

    pub(crate) async fn emit_notifications_for_execution(
        &self,
        session: &ChatSession,
        execution: &ChatGraphExecution,
    ) -> Result<(), AppError> {
        if execution.response.degrade_trace.is_empty() {
            return Ok(());
        }

        let (title, body) = match execution.mode.as_str() {
            "general" => (
                "General mode degraded",
                "General mode used a degraded path for the latest turn.",
            ),
            "search" => (
                "Search mode degraded",
                "Search mode could not complete a real provider-backed search.",
            ),
            _ => (
                "RAG mode degraded",
                "RAG mode used a degraded retrieval or synthesis path.",
            ),
        };

        let _ = self
            .emit_notification(
                "system.degrade",
                title,
                body,
                serde_json::json!({
                    "agent_type": execution.mode,
                    "session_id": session.id,
                    "reasons": execution.response.degrade_trace.iter().map(|item| item.reason.clone()).collect::<Vec<_>>(),
                }),
            )
            .await;
        Ok(())
    }
}
