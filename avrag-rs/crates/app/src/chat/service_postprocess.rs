impl AppState {
    pub(crate) async fn apply_output_guard_to_execution(
        &self,
        session: &ChatSession,
        execution: &mut ChatExecution,
        trace_id: &str,
        user_uuid: Uuid,
        pg: Option<&PgAppRepository>,
    ) -> Result<(), AppError> {
        if !execution.apply_output_guard {
            return Ok(());
        }

        let (sanitized_answer, guard_report) = self.guard_pipeline.check_output(
            &execution.response.answer,
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
        execution: &mut ChatExecution,
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
                ChatTurn {
                    user_content: req.query.trim(),
                    assistant_content: &execution.response.answer,
                    assistant_answer_blocks: &execution.response.answer_blocks,
                    agent_type: &req.agent_type,
                    citations: &execution.response.citations,
                },
            )
            .await
            .map_err(map_pg_error)?;
        execution.response.message_id = Some(assistant_message_id);

        let summary_updated = self.maybe_update_session_summary(pg, session).await;
        let _ = self
            .remember_explicit_agent_preference(req.query.trim())
            .await;

        if is_direct_chat_mode(&execution.mode)
            && let Some(ref cm) = self.chatmemory
            && let Ok(messages) = pg.list_messages(&self.auth, session_uuid).await
        {
            let raw_custom_preferences =
                if let Some(user_id) = self.auth.actor_id().map(|value| value.into_uuid()) {
                    pg.get_user_profile(&self.auth, user_id)
                        .await
                        .ok()
                        .flatten()
                        .map(|profile| profile.custom_preferences)
                        .unwrap_or_else(|| serde_json::json!({}))
                } else {
                    serde_json::json!({})
                };
            let agent_memory = self
                .current_user_preferences()
                .await
                .ok()
                .map(|preferences| preferences.agent_memory)
                .unwrap_or_default();
            let custom_preferences = merge_general_profile_custom_preferences(
                raw_custom_preferences,
                agent_memory,
                req.query.trim(),
                &execution.input_usage_text,
            );
            let structured_profile = if let Some(user_id) = self.auth.actor_id().map(|value| value.into_uuid()) {
                pg.get_user_profile(&self.auth, user_id)
                    .await
                    .ok()
                    .flatten()
                    .map(|profile| profile.structured_profile)
                    .unwrap_or_else(|| serde_json::json!({}))
            } else {
                serde_json::json!({})
            };
            let _ = cm
                .update_user_profile(
                    &self.auth,
                    avrag_chatmemory::UserProfileUpdate {
                        expertise_domains: derive_profile_domains(&messages, req.query.trim()),
                        preferred_answer_style: detect_preferred_style(req.query.trim()),
                        frequently_asked_topics: derive_profile_topics(&messages, req.query.trim()),
                        custom_preferences,
                        structured_profile,
                        inference_version: "general-v1".to_string(),
                    },
                )
                .await;
        }

        if summary_updated {
            // Build summary once and pass it to the dream layer.
            let session_uuid =
                parse_uuid_or_app_error(&session.id, "session_not_found", "session not found")
                    .unwrap_or_else(|_| uuid::Uuid::nil());
            if let Ok(messages) = pg.list_messages(&self.auth, session_uuid).await {
                let summary = self.build_session_summary(&messages).await;
                if !summary.trim().is_empty() {
                    let _ = self
                        .maybe_update_structured_profile(pg, session, &summary)
                        .await;
                }
            }
        }

        if is_direct_chat_mode(&execution.mode)
            && summary_updated
                && let Some(mode_debug) = execution.response.mode_debug.as_mut()
                && let Some(general) = mode_debug.general.as_mut()
            {
                general.insert("summary_updated".to_string(), serde_json::json!(true));
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
            } else {
                analytics::Surface::Workspace
            },
            result,
            Uuid::parse_str(&session.id).ok(),
            Uuid::parse_str(&session.notebook_id).ok(),
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

        if let Some(ref usage_svc) = self.quota_manager
            && let Some(ref llm_usage) = execution.llm_usage {
                let feature = match execution.mode.as_str() {
                    "chat" | "general" => avrag_billing::usage_limit::BillableFeature::Chat,
                    "search" => avrag_billing::usage_limit::BillableFeature::Search,
                    "rag" => avrag_billing::usage_limit::BillableFeature::Answer,
                    _ => avrag_billing::usage_limit::BillableFeature::Chat,
                };
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
                    stage: format!("{}_chat", execution.mode),
                    session_id: None,
                    document_id: None,
                    request_id: self.auth.request_id().map(|s| s.to_string()),
                    trace_id: None,
                };
                let _ = usage_svc
                    .rolling_service()
                    .record_usage(
                        &ctx,
                        avrag_billing::usage_limit::UsageRecord {
                            provider: if llm_usage.provider.trim().is_empty() {
                                "unknown"
                            } else {
                                llm_usage.provider.as_str()
                            },
                            model: if llm_usage.model.trim().is_empty() {
                                "unknown"
                            } else {
                                llm_usage.model.as_str()
                            },
                            prompt_tokens: llm_usage.prompt_tokens,
                            completion_tokens: llm_usage.completion_tokens,
                            total_tokens: llm_usage.total_tokens,
                            usage_source: avrag_billing::usage_limit::UsageSource::Actual,
                        },
                    )
                    .await;
            }

        if let Some(ref llm_usage) = execution.llm_usage {
            let feature = match execution.mode.as_str() {
                "chat" | "general" => "chat",
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
                && let Some(tool_telemetry) = debug_metadata.get("tool_telemetry") {
                    cost_metadata["tool_telemetry"] = tool_telemetry.clone();
                }
            self.record_cost_event_if_available(
                crate::lib_impl::state_methods::CostEventRecord {
                    event_name: analytics::CostEventName::LlmUsageMetered,
                    feature,
                    session_id: Uuid::parse_str(&execution.response.session_id).ok(),
                    notebook_id: None,
                    usage: llm_usage,
                    source: "pipeline",
                    metadata: cost_metadata,
                },
            )
            .await;
        }

        Ok(())
    }

    pub(crate) async fn emit_notifications_for_execution(
        &self,
        session: &ChatSession,
        execution: &ChatExecution,
    ) -> Result<(), AppError> {
        if execution.response.degrade_trace.is_empty() {
            return Ok(());
        }

        let (title, body) = match execution.mode.as_str() {
            mode if is_direct_chat_mode(mode) => (
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

fn is_direct_chat_mode(mode: &str) -> bool {
    matches!(mode, "chat" | "general")
}
