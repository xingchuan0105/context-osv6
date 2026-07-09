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

        let (sanitized_answer, guard_report) = self.orchestrator.guard_pipeline().check_output(
            &execution.response.answer,
            Some(trace_id.to_string()),
        );

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
        let tool_results: Vec<contracts::ToolResult> = execution.response.tool_results.iter().map(|r| {
            contracts::ToolResult::from(r.clone())
        }).collect();
        // ADR-0010: server-side query normalization removed; no per-turn
        // resolved_query or query_resolution metadata is persisted.
        let user_turn_metadata: Option<serde_json::Value> = None;
        let user_resolved_query: Option<&str> = None;
        let assistant_message_id = chat_persistence
            .append_chat_turn(
                &self.auth,
                session_uuid,
                app_core::AppendChatTurn {
                    user_content: req.query.trim(),
                    assistant_content: &execution.response.answer,
                    assistant_answer_blocks: &execution.response.answer_blocks,
                    agent_type: &req.agent_type,
                    citations: &execution.response.citations,
                    tool_results: &tool_results,
                    user_turn_metadata,
                    user_resolved_query,
                },
            )
            .await?;
        execution.response.message_id = Some(assistant_message_id);

        let _ = self
            .remember_explicit_agent_preference(req.query.trim())
            .await;

        if is_direct_chat_mode(&execution.mode)
            && let Some(ref cm) = self.orchestrator.chatmemory()
            && let Ok(messages) = chat_persistence.list_messages(&self.auth, session_uuid).await
            && let Some(user_id) = self.auth.actor_id().map(|value| value.into_uuid())
        {
            let existing_profile = chat_persistence
                .get_user_profile(&self.auth, user_id)
                .await
                .ok()
                .flatten();

            let should_update = match &existing_profile {
                Some(profile) => {
                    let since_last =
                        chrono::Utc::now().signed_duration_since(profile.inferred_at);
                    since_last.num_hours() >= 24
                }
                None => true,
            };

            if should_update {
                let raw_custom_preferences = existing_profile
                    .as_ref()
                    .map(|p| p.custom_preferences.clone())
                    .unwrap_or_else(|| serde_json::json!({}));
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
                let structured_profile = existing_profile
                    .as_ref()
                    .map(|p| p.structured_profile.clone())
                    .unwrap_or_else(|| serde_json::json!({}));
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
        }

        if let Ok(messages) = chat_persistence.list_messages(&self.auth, session_uuid).await {
            let recent_turns = build_recent_turns_context(&messages, PROFILE_INPUT_TURN_WINDOW);
            if !recent_turns.trim().is_empty() {
                let profile_updated = self
                    .maybe_update_structured_profile(chat_persistence, session, &recent_turns)
                    .await;
                if profile_updated
                    && is_direct_chat_mode(&execution.mode)
                    && let Some(mode_debug) = execution.response.mode_debug.as_mut()
                    && let Some(general) = mode_debug.general.as_mut()
                {
                    general.insert("profile_updated".to_string(), serde_json::json!(true));
                }
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
            Uuid::parse_str(&session.workspace_id).ok(),
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
                && let Some(tool_telemetry) = debug_metadata.get("tool_telemetry") {
                    cost_metadata["tool_telemetry"] = tool_telemetry.clone();
                }
            self.record_cost_event_if_available(
                app_billing::CostEventRecord {
                    event_name: analytics::CostEventName::LlmUsageMetered,
                    feature,
                    session_id: Uuid::parse_str(&execution.response.session_id).ok(),
                    workspace_id: None,
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
    matches!(mode, "chat")
}

/// Recent raw turns fed to the L3 dream layer (not session-summary).
const PROFILE_INPUT_TURN_WINDOW: usize = 12;

fn build_recent_turns_context(messages: &[contracts::chat::ChatMessage], max_turns: usize) -> String {
    messages
        .iter()
        .rev()
        .take(max_turns)
        .rev()
        .map(|item| format!("{}: {}", item.role, item.content))
        .collect::<Vec<_>>()
        .join("\n")
}
