impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let auth = auth_context_from_config(&config);
        let object_store = Arc::new(ObjectStoreHandle::local(PathBuf::from(
            config.object_root.clone(),
        )));
        let llm_client = make_llm_client(&config.answer_llm);
        let summary_llm_client =
            make_llm_client(&config.summary_llm).or_else(|| make_llm_client(&config.answer_llm));
        let chatmemory = None;
        let search_executor = Some(Arc::new(SearchExecutor::new(avrag_search::SearchConfig {
            provider: config.search.provider.clone(),
            base_url: config.search.base_url.clone(),
            api_key: config.search.api_key.clone(),
            max_results: config.search.max_results,
            perplexity_api_key: config.search.perplexity_api_key.clone(),
        })));
        let agent_service = Some(build_unified_agent_service(
            llm_client.clone(),
            config.answer_llm.temperature,
            search_executor.clone(),
            None,
        ));

        // RAG components not available in memory mode
        Self {
            config,
            auth,
            pg: None,
            inner: Arc::new(RwLock::new(MemoryState::default())),
            llm_client,
            summary_llm_client,
            chatmemory,
            analytics: None,
            search_executor,
            rag_runtime: None,
            agent_service,
            object_store,
            guard_pipeline: Arc::new(GuardPipeline::new()),
            usage_limit: None,
            uses_memory_adapters: true,
        }
    }

    pub async fn bootstrap(config: AppConfig) -> AnyResult<Self> {
        let auth = auth_context_from_config(&config);
        let object_store = Arc::new(build_object_store(&config).await?);
        let pg = if let Some(database_url) = config.database_url.as_deref() {
            let repository = PgAppRepository::connect(database_url).await?;
            if config.auto_migrate {
                repository.migrate().await?;
            }
            Some(Arc::new(repository))
        } else {
            None
        };

        let llm_client = make_llm_client(&config.answer_llm);
        let summary_llm_client =
            make_llm_client(&config.summary_llm).or_else(|| make_llm_client(&config.answer_llm));
        let chatmemory = pg.as_ref().map(|p| Arc::new(ChatMemory::new(p.clone())));
        let search_executor = Some(Arc::new(SearchExecutor::new(avrag_search::SearchConfig {
            provider: config.search.provider.clone(),
            base_url: config.search.base_url.clone(),
            api_key: config.search.api_key.clone(),
            max_results: config.search.max_results,
            perplexity_api_key: config.search.perplexity_api_key.clone(),
        })));

        // Create RAG components if pg and embedding are available
        let rag_runtime = if let Some(ref pg_repo) = pg {
            let embedding = make_embedding_client(&config.embedding);
            let mm_embedding = make_embedding_client(&config.mm_embedding);
            let planner = make_planner(&config.intent_llm);
            let synthesizer = make_synthesizer(&config.answer_llm);
            let reranker = make_reranker(&config.rerank);
            let mm_reranker = make_reranker(&config.mm_rerank);

            let fallback_embedding = Arc::new(EmbeddingClient::new(avrag_llm::ModelProviderConfig {
                base_url: "https://api.siliconflow.cn/v1".to_string(),
                api_key: String::new(),
                model: "Qwen/Qwen3-Embedding-8B".to_string(),
                timeout_ms: 15000,
                api_style: None,
                dimensions: None,
                enable_thinking: None,
            }));
            let embedding_for_config = embedding.unwrap_or(fallback_embedding);
            let attach_rag_components = |mut rag_config: RagConfig| {
                if let Some(p) = planner.clone() {
                    rag_config = rag_config.with_planner(p);
                }
                if let Some(s) = synthesizer.clone() {
                    rag_config = rag_config.with_synthesizer(s);
                }
                if let Some(mm) = mm_embedding.clone() {
                    rag_config = rag_config.with_mm_embedding(mm);
                }
                if let Some(r) = reranker.clone() {
                    rag_config = rag_config.with_reranker(r);
                }
                if let Some(mm_r) = mm_reranker.clone() {
                    rag_config = rag_config.with_mm_reranker(mm_r);
                }
                rag_config
            };

            let rag_config = attach_rag_components(RagConfig::new_for_data_plane(
                embedding_for_config,
                Some(pg_repo.clone()),
            ));
            let milvus_config = StorageMilvusConfig {
                url: config.milvus.url.clone(),
                token: Some(config.milvus.token.clone()).filter(|token| !token.trim().is_empty()),
                database: Some(config.milvus.database.clone())
                    .filter(|database| !database.trim().is_empty()),
                collection_prefix: config.milvus.collection_prefix.clone(),
                text_vector_dim: config.milvus.text_vector_dim,
                multimodal_vector_dim: config.milvus.multimodal_vector_dim,
                metric_type: config.milvus.metric_type.clone(),
            };
            let data_plane: Arc<dyn RetrievalDataPlane> =
                Arc::new(MilvusDataPlane::new(milvus_config));
            data_plane.ensure_schema().await?;
            Some(Arc::new(RagRuntime::with_data_plane(
                rag_config, data_plane,
            )))
        } else {
            None
        };

        let usage_limit = pg
            .as_ref()
            .map(|p| Arc::new(avrag_usage_limit::UsageLimitService::new(p.raw().clone())));
        let analytics = pg
            .as_ref()
            .map(|p| Arc::new(analytics::AnalyticsService::new(p.raw().clone())));
        let uses_memory_adapters = pg.is_none();
        let agent_service = Some(build_unified_agent_service(
            llm_client.clone(),
            config.answer_llm.temperature,
            search_executor.clone(),
            rag_runtime.clone(),
        ));

        Ok(Self {
            config,
            auth,
            pg,
            inner: Arc::new(RwLock::new(MemoryState::default())),
            llm_client,
            summary_llm_client,
            chatmemory,
            analytics,
            search_executor,
            rag_runtime,
            agent_service,
            object_store,
            guard_pipeline: Arc::new(GuardPipeline::new()),
            usage_limit,
            uses_memory_adapters,
        })
    }

    pub async fn load_docscope_metadata(
        &self,
        doc_scope: &[String],
    ) -> Result<common::DocScopeMetadata, AppError> {
        let pg = self
            .pg
            .as_ref()
            .ok_or_else(|| AppError::internal("postgres backend is not configured"))?;

        let doc_uuids: Vec<Uuid> = doc_scope
            .iter()
            .filter_map(|id| Uuid::parse_str(id).ok())
            .collect();

        let metadata = pg
            .get_summary_metadata(&self.auth, &doc_uuids)
            .await
            .map_err(map_pg_error)?;

        Ok(build_docscope_metadata(metadata))
    }

    pub async fn build_session_context(
        &self,
        session: &common::ChatSession,
    ) -> Result<Option<RagSessionContext>, AppError> {
        let session_uuid = uuid::Uuid::parse_str(&session.id).map_err(|_| {
            AppError::validation("invalid_session_id", "invalid session UUID format")
        })?;

        let pg = self
            .pg
            .as_ref()
            .ok_or_else(|| AppError::internal("postgres backend is not configured"))?;

        let messages = pg
            .list_messages(&self.auth, session_uuid)
            .await
            .unwrap_or_default();
        if messages.is_empty() {
            return Ok(None);
        }

        let summary = if let Some(cm) = &self.chatmemory {
            cm.load(&self.auth, session_uuid)
                .await
                .ok()
                .and_then(|m| m.layer2.map(|l2| l2.summary))
        } else {
            None
        };

        Ok(Self::build_rag_session_context(messages, summary))
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub fn auth(&self) -> &AuthContext {
        &self.auth
    }

    pub fn with_auth(&self, auth: AuthContext) -> Self {
        let mut config = self.config.clone();
        config.org_id = auth.org_id().into_uuid().to_string();
        if let Some(actor_id) = auth.actor_id() {
            config.user_id = actor_id.into_uuid().to_string();
        }

        Self {
            config,
            auth,
            pg: self.pg.clone(),
            inner: self.inner.clone(),
            llm_client: self.llm_client.clone(),
            summary_llm_client: self.summary_llm_client.clone(),
            chatmemory: self.chatmemory.clone(),
            analytics: self.analytics.clone(),
            search_executor: self.search_executor.clone(),
            rag_runtime: self.rag_runtime.clone(),
            agent_service: self.agent_service.clone(),
            object_store: self.object_store.clone(),
            guard_pipeline: self.guard_pipeline.clone(),
            usage_limit: self.usage_limit.clone(),
            uses_memory_adapters: self.uses_memory_adapters,
        }
    }

    pub fn runtime_mode(&self) -> &'static str {
        if self.pg.is_some() {
            "postgres"
        } else {
            "memory"
        }
    }

    pub fn uses_memory_adapters(&self) -> bool {
        self.uses_memory_adapters
    }

    pub async fn pg_ready(&self) -> bool {
        if let Some(pg) = &self.pg {
            return pg.ping().await.is_ok();
        }
        false
    }

    pub fn pg(&self) -> Option<Arc<PgAppRepository>> {
        self.pg.clone()
    }

    pub fn agent_service(&self) -> Option<Arc<UnifiedAgentService>> {
        self.agent_service.clone()
    }

    pub fn set_agent_service(&mut self, service: UnifiedAgentService) {
        self.agent_service = Some(Arc::new(service));
    }

    /// Build an `AgentRequest` from chat request and memory context.
    /// This is the single conversion point from legacy `ChatRequest` to new agent protocol.
    pub async fn build_agent_request(
        &self,
        req: &common::ChatRequest,
        kind: crate::agents::AgentKind,
    ) -> crate::agents::runtime::AgentRequest {
        let notebook_id = req.notebook_id.clone();
        let session_id = req.session_id.clone();
        let doc_scope = req.doc_scope.clone();
        let stream = req.stream;

        let memory_context = if let (Some(sid), Some(cm)) = (&session_id, &self.chatmemory) {
            if let Ok(session_uuid) = uuid::Uuid::parse_str(sid) {
                cm.load(&self.auth, session_uuid)
                    .await
                    .ok()
            } else {
                None
            }
        } else {
            None
        };
        let session_summary = memory_context
            .as_ref()
            .and_then(|memory| memory.layer2.as_ref().map(|layer2| layer2.summary.clone()));
        let user_preferences = memory_context
            .as_ref()
            .and_then(|memory| memory.layer3.as_ref().map(agent_user_preferences_json));
        let working_memory = memory_context
            .as_ref()
            .and_then(|memory| memory.working_memory.as_ref().map(agent_working_memory_json));

        crate::agents::runtime::AgentRequest {
            kind,
            query: req.query.clone(),
            notebook_id,
            session_id,
            doc_scope,
            messages: req.messages.clone(),
            session_summary,
            user_preferences,
            working_memory,
            debug: false,
            stream,
            auth_context: serde_json::json!({
                "org_id": self.auth.org_id().to_string(),
                "user_id": self.auth.actor_id().map(|a| a.into_uuid().to_string()).unwrap_or_default(),
            }),
            metadata: std::collections::BTreeMap::new(),
        }
    }

    pub(crate) fn build_general_agent_debug(
        &self,
        agent_request: &crate::agents::runtime::AgentRequest,
    ) -> BTreeMap<String, serde_json::Value> {
        let mut general_debug = BTreeMap::new();
        general_debug.insert(
            "agent_kind".to_string(),
            serde_json::json!(crate::agents::AgentKind::Chat.as_canonical_str()),
        );
        general_debug.insert(
            "memory_loaded".to_string(),
            serde_json::json!(agent_request.session_summary.is_some()
                || agent_request.user_preferences.is_some()
                || agent_request.working_memory.is_some()),
        );
        general_debug.insert("summary_updated".to_string(), serde_json::json!(false));
        general_debug.insert(
            "has_profile".to_string(),
            serde_json::json!(agent_request.user_preferences.is_some()),
        );
        general_debug.insert(
            "has_working_memory".to_string(),
            serde_json::json!(agent_request.working_memory.is_some()),
        );
        general_debug
    }

    pub fn analytics(&self) -> Option<Arc<analytics::AnalyticsService>> {
        self.analytics.clone()
    }

    pub async fn record_product_event_if_available(
        &self,
        event_name: analytics::ProductEventName,
        surface: analytics::Surface,
        result: analytics::ResultTag,
        session_id: Option<Uuid>,
        notebook_id: Option<Uuid>,
        metadata: serde_json::Value,
    ) {
        let Some(ref analytics) = self.analytics else {
            return;
        };
        let Some(user_id) = self.auth.actor_id().map(|actor| actor.into_uuid()) else {
            return;
        };

        let event = analytics::ProductEvent {
            event_id: Uuid::new_v4(),
            event_time: chrono::Utc::now(),
            user_id,
            session_id,
            notebook_id,
            surface,
            event_name,
            result,
            request_id: self.auth.request_id().map(str::to_string),
            trace_id: None,
            client_platform: "web".to_string(),
            metadata,
        };
        if let Err(error) = analytics.record_product_event(&event).await {
            telemetry::prometheus::record_dependency_failure("analytics");
            tracing::warn!(error = %error, event_name = ?event_name, "failed to record product event");
        }
    }

    pub async fn record_cost_event_if_available(
        &self,
        event_name: analytics::CostEventName,
        feature: &str,
        session_id: Option<Uuid>,
        notebook_id: Option<Uuid>,
        usage: &avrag_llm::LlmUsage,
        source: &str,
        metadata: serde_json::Value,
    ) {
        let Some(ref analytics) = self.analytics else {
            return;
        };
        let Some(user_id) = self.auth.actor_id().map(|actor| actor.into_uuid()) else {
            return;
        };

        let event = analytics::CostEvent {
            event_id: Uuid::new_v4(),
            event_time: chrono::Utc::now(),
            user_id,
            session_id,
            notebook_id,
            event_name,
            feature: feature.to_string(),
            provider: non_empty_or_unknown(&usage.provider),
            model: non_empty_or_unknown(&usage.model),
            prompt_tokens: i64::from(usage.prompt_tokens),
            completion_tokens: i64::from(usage.completion_tokens),
            embedding_tokens: 0,
            usage_units: avrag_usage_limit::compute_usage_units(
                &usage.provider,
                &usage.model,
                usage.prompt_tokens,
                usage.completion_tokens,
            ),
            storage_bytes_delta: 0,
            external_call_count: 0,
            source: source.to_string(),
            metadata,
        };
        if let Err(error) = analytics.record_cost_event(&event).await {
            telemetry::prometheus::record_dependency_failure("analytics");
            tracing::warn!(error = %error, event_name = ?event_name, "failed to record cost event");
        }
    }

    pub async fn record_storage_cost_event_if_available(
        &self,
        event_name: analytics::CostEventName,
        feature: &str,
        notebook_id: Option<Uuid>,
        storage_bytes_delta: i64,
        source: &str,
        metadata: serde_json::Value,
    ) {
        let Some(ref analytics) = self.analytics else {
            return;
        };
        let Some(user_id) = self.auth.actor_id().map(|actor| actor.into_uuid()) else {
            return;
        };

        let event = analytics::CostEvent {
            event_id: Uuid::new_v4(),
            event_time: chrono::Utc::now(),
            user_id,
            session_id: None,
            notebook_id,
            event_name,
            feature: feature.to_string(),
            provider: "internal".to_string(),
            model: "storage".to_string(),
            prompt_tokens: 0,
            completion_tokens: 0,
            embedding_tokens: 0,
            usage_units: 0,
            storage_bytes_delta,
            external_call_count: 0,
            source: source.to_string(),
            metadata,
        };
        if let Err(error) = analytics.record_cost_event(&event).await {
            telemetry::prometheus::record_dependency_failure("analytics");
            tracing::warn!(error = %error, event_name = ?event_name, "failed to record storage cost event");
        }
    }

    pub async fn record_external_search_cost_event_if_available(
        &self,
        provider: &str,
        model: &str,
        notebook_id: Option<Uuid>,
        external_call_count: i64,
        metadata: serde_json::Value,
    ) {
        let Some(ref analytics) = self.analytics else {
            return;
        };
        let Some(user_id) = self.auth.actor_id().map(|actor| actor.into_uuid()) else {
            return;
        };

        let event = analytics::CostEvent {
            event_id: Uuid::new_v4(),
            event_time: chrono::Utc::now(),
            user_id,
            session_id: None,
            notebook_id,
            event_name: analytics::CostEventName::ExternalSearchUsageMetered,
            feature: "search".to_string(),
            provider: non_empty_or_unknown(provider),
            model: non_empty_or_unknown(model),
            prompt_tokens: 0,
            completion_tokens: 0,
            embedding_tokens: 0,
            usage_units: 0,
            storage_bytes_delta: 0,
            external_call_count,
            source: "external_search".to_string(),
            metadata,
        };
        if let Err(error) = analytics.record_cost_event(&event).await {
            telemetry::prometheus::record_dependency_failure("analytics");
            tracing::warn!(error = %error, "failed to record external search cost event");
        }
    }
}

fn non_empty_or_unknown(value: &str) -> String {
    if value.trim().is_empty() {
        "unknown".to_string()
    } else {
        value.to_string()
    }
}

fn agent_user_preferences_json(profile: &avrag_chatmemory::Layer3Profile) -> serde_json::Value {
    serde_json::json!({
        "expertise_domains": profile.expertise_domains.clone(),
        "preferred_answer_style": profile.preferred_answer_style.clone(),
        "frequently_asked_topics": profile.frequently_asked_topics.clone(),
        "custom_preferences": profile.custom_preferences.clone(),
        "inference_version": profile.inference_version.clone(),
    })
}

fn agent_working_memory_json(working: &avrag_chatmemory::WorkingMemory) -> serde_json::Value {
    serde_json::json!({
        "state_type": working.state_type.clone(),
        "current_topic": working.current_topic.clone(),
        "last_document": working.last_document.clone(),
        "last_entity": working.last_entity.clone(),
        "unresolved_question": working.unresolved_question.clone(),
        "pending_questions": working.pending_questions.clone(),
        "gathered_facts": working.gathered_facts.clone(),
        "confidence_score": working.confidence_score,
        "state_history": working.state_history.clone(),
        "last_updated_at": working.last_updated_at.to_rfc3339(),
    })
}
