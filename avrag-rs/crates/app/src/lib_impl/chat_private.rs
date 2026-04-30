impl AppState {
    async fn list_ready_documents_for_chat(
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

    fn build_rag_session_context(
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

    async fn maybe_update_session_summary(
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

    async fn build_session_summary(&self, messages: &[ChatMessage]) -> String {
        let summary_prompt = "Summarize the following conversation in 2-3 concise sentences. Preserve user goals, constraints, and unresolved questions.";
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
                &self.summary_llm_client,
                self.config.summary_llm.temperature.or(Some(0.2)),
            ),
            (
                &self.llm_client,
                self.config.answer_llm.temperature.or(Some(0.2)),
            ),
        ] {
            if let Some(client) = llm {
                if let Ok(response) = client
                    .complete(
                        &[
                            avrag_llm::ChatMessage::system(summary_prompt),
                            avrag_llm::ChatMessage::user(&prompt),
                        ],
                        temperature,
                    )
                    .await
                {
                    if !response.content.trim().is_empty() {
                        self.record_llm_usage_if_available(
                            avrag_usage_limit::BillableFeature::Summary,
                            "session_summary",
                            &response.usage,
                            "inline",
                        )
                        .await;
                        return response.content.trim().to_string();
                    }
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

    async fn emit_notification(
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
        feature: avrag_usage_limit::BillableFeature,
        stage: &str,
        usage: &avrag_llm::LlmUsage,
        source: &str,
    ) {
        if let Some(ref svc) = self.usage_limit {
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
                stage: stage.to_string(),
                session_id: None,
                document_id: None,
                request_id: self.auth.request_id().map(|s| s.to_string()),
                trace_id: None,
            };
            let _ = svc
                .record_usage(
                    &ctx,
                    &non_empty_or_unknown(&usage.provider),
                    &non_empty_or_unknown(&usage.model),
                    usage.prompt_tokens,
                    usage.completion_tokens,
                    usage.total_tokens,
                    avrag_usage_limit::UsageSource::Actual,
                )
                .await;
        }
        self.record_cost_event_if_available(
            analytics::CostEventName::LlmUsageMetered,
            feature.as_str(),
            None,
            None,
            usage,
            source,
            serde_json::json!({
                "stage": stage,
                "feature": feature.as_str(),
            }),
        )
        .await;
    }

    /// Get usage limit response for the current user.
    pub async fn get_user_usage_limit(
        &self,
    ) -> Result<avrag_usage_limit::UsageLimitResponse, AppError> {
        let Some(ref svc) = self.usage_limit else {
            return Err(AppError::internal("usage limit service not configured"));
        };
        let user_id = self
            .auth
            .actor_id()
            .map(|a| a.into_uuid())
            .ok_or_else(|| AppError::internal("no authenticated user"))?;
        let org_id = self.auth.org_id().into_uuid();
        svc.get_user_usage(org_id, user_id).await.map_err(|e| {
            AppError::internal(&format!("failed to get usage limit: {}", e))
        })
    }

    /// Check if the current user has quota remaining.
    pub async fn check_user_quota(&self) -> Result<avrag_usage_limit::QuotaCheckResult, AppError> {
        let Some(ref svc) = self.usage_limit else {
            return Err(AppError::internal("usage limit service not configured"));
        };
        let user_id = self
            .auth
            .actor_id()
            .map(|a| a.into_uuid())
            .unwrap_or_else(Uuid::nil);
        let org_id = self.auth.org_id().into_uuid();
        svc.check_quota(org_id, user_id).await.map_err(|e| {
            AppError::internal(&format!("usage limit check failed: {}", e))
        })
    }

    async fn ensure_metric_quota(&self, metric_type: &str, requested: i64) -> Result<(), AppError> {
        if requested <= 0 {
            return Ok(());
        }
        let Some(pg) = &self.pg else {
            return Ok(());
        };
        let decision = billing::check_quota(pg.clone(), self.auth.org_id(), metric_type, requested)
            .await
            .map_err(map_anyhow_error)?;
        if decision.allowed {
            return Ok(());
        }
        Err(AppError::rate_limited(
            "quota_exceeded",
            format!(
                "quota exceeded for {} on plan {} (current={}, requested={}, hard_limit={})",
                decision.metric_type,
                decision.plan_id,
                decision.current_usage,
                decision.requested,
                decision
                    .hard_limit
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "unlimited".to_string())
            ),
            decision.retry_after_secs,
        ))
    }

    async fn record_usage(
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

    fn current_org_id(&self) -> String {
        self.auth.org_id().to_string()
    }

    fn current_user_id(&self) -> String {
        self.auth
            .actor_id()
            .map(|actor_id| actor_id.into_uuid().to_string())
            .unwrap_or_else(|| self.config.user_id.clone())
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
                + self.config.object_storage.upload_url_expire_sec
        });
        let signature =
            sign_upload_payload(&upload_signing_secret(), document_id, object_path, expires)?;
        Ok(format!(
            "{}/uploads/{}?expires={}&signature={}",
            self.config.public_base_url, document_id, expires, signature
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

    fn object_root_path(&self) -> &Path {
        Path::new(&self.config.object_root)
    }

    async fn enqueue_ingest_task(&self, seed: DocumentTaskSeed) -> Result<(), AppError> {
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
                        ingestion::IngestionTaskPayload::ReindexDocument(_) => String::new(),
                    }
                }),
            ))
            .await
            .map_err(map_pg_error)?;
        }
        Ok(())
    }

    async fn enqueue_reindex_task(&self, seed: DocumentTaskSeed) -> Result<(), AppError> {
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

    fn memory_session_visible(&self, state: &MemoryState, session: &ChatSession) -> bool {
        state
            .notebooks
            .get(&session.notebook_id)
            .map(|notebook| notebook.org_id == self.current_org_id())
            .unwrap_or(false)
    }

}
