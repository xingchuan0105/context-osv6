use crate::agents::service::UnifiedAgentService;
use crate::lib_impl::*;
use anyhow::Result as AnyResult;
use avrag_auth::AuthContext;
use avrag_chatmemory::ChatMemory;
use avrag_guardrails::GuardPipeline;
use avrag_llm::EmbeddingClient;
use avrag_rag_core::{
    RagConfig, RagRuntime, RetrievalDataPlane, context::SessionContext as RagSessionContext,
};
use avrag_search::SearchExecutor;
use avrag_storage_milvus::{MilvusConfig as StorageMilvusConfig, MilvusDataPlane};
use avrag_storage_pg::{ObjectStoreHandle, PgAppRepository};
use common::AppError;
use common::key_vault::EnvKeyVault;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;

fn build_key_vault_from_config(config: &AppConfig) -> Arc<dyn common::key_vault::KeyVault> {
    let vault = EnvKeyVault::new()
        .with_entry("agent_llm_api_key", config.agent_llm.api_key.clone())
        .with_entry("memory_llm_api_key", config.memory_llm.api_key.clone())
        .with_entry("embedding_api_key", config.embedding.api_key.clone())
        .with_entry("mm_embedding_api_key", config.mm_embedding.api_key.clone())
        .with_entry("search_api_key", config.search.api_key.clone())
        .with_entry(
            "ingestion_llm_api_key",
            config.ingestion_llm.api_key.clone(),
        );
    Arc::new(vault)
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let auth = auth_context_from_config(&config);
        let object_store = Arc::new(ObjectStoreHandle::local(PathBuf::from(
            config.object_root.clone(),
        )));
        let llm_client = make_llm_client(&config.agent_llm);
        let memory_llm_client = make_llm_client(&config.memory_llm);
        let chatmemory = None;
        let search_executor = Some(Arc::new(SearchExecutor::new(avrag_search::SearchConfig {
            provider: config.search.provider.clone(),
            base_url: config.search.base_url.clone(),
            api_key: config.search.api_key.clone(),
            max_results: config.search.max_results,
            search_lang: config.search.search_lang.clone(),
            country: config.search.country.clone(),
            freshness: config.search.freshness.clone(),
        })));
        let agent_service = Some(build_unified_agent_service(
            llm_client.clone(),
            config.agent_llm.temperature,
            search_executor.clone(),
            None,
            &config.prompts.dir,
        ));
        let key_vault = build_key_vault_from_config(&config);

        // RAG components not available in memory mode
        Self {
            // config,  // REMOVED
            auth,
            pg: None,
            inner: Arc::new(RwLock::new(MemoryState::default())),
            llm_client,
            memory_llm_client,
            chatmemory,
            analytics: None,
            rag_runtime: None,
            agent_service,
            object_store,
            guard_pipeline: Arc::new(GuardPipeline::new()),
            quota_manager: None,
            uses_memory_adapters: true,
            // 提取非敏感配置
            public_base_url: config.public_base_url,
            object_root: config.object_root,
            usage_limit_phase: config.usage_limit.enforcement_phase,
            search_provider: config.search.provider,
            search_mode: config.search.mode,
            redis_url: config.redis.url.clone(),
            object_storage_upload_expire_sec: config.object_storage.upload_url_expire_sec,
            object_storage_download_expire_sec: config.object_storage.download_url_expire_sec,
            max_upload_file_size_bytes: config.max_upload_file_size_bytes,
            api_keys: Arc::new(RwLock::new(BTreeMap::new())),
            key_vault,
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

        let llm_client = make_llm_client(&config.agent_llm);
        let memory_llm_client = make_llm_client(&config.memory_llm);
        let chatmemory = pg.as_ref().map(|p| Arc::new(ChatMemory::new(p.clone())));
        let search_executor = Some(Arc::new(SearchExecutor::new(avrag_search::SearchConfig {
            provider: config.search.provider.clone(),
            base_url: config.search.base_url.clone(),
            api_key: config.search.api_key.clone(),
            max_results: config.search.max_results,
            search_lang: config.search.search_lang.clone(),
            country: config.search.country.clone(),
            freshness: config.search.freshness.clone(),
        })));

        // Create shared cache store if Redis URL is non-empty
        let cache_store = if config.redis.url.trim().is_empty() {
            None
        } else {
            avrag_cache_redis::CacheStore::new(&config.redis.url)
                .ok()
                .map(Arc::new)
        };

        // Create RAG components if pg, embedding, and enable_rag are all available
        let rag_runtime = if config.enable_rag && pg.is_some() {
            let pg_repo = pg.as_ref().unwrap();
            let embedding = make_embedding_client(&config.embedding, cache_store.clone());
            let mm_embedding = make_embedding_client(&config.mm_embedding, cache_store.clone());
            let planner = make_planner(&config.agent_llm, cache_store.clone());
            let reranker = make_reranker(&config.rerank);
            let mm_reranker = make_reranker(&config.mm_rerank);

            let fallback_embedding =
                Arc::new(EmbeddingClient::new(avrag_llm::ModelProviderConfig {
                    base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
                    api_key: String::new(),
                    model: "text-embedding-v4".to_string(),
                    timeout_ms: 15000,
                    api_style: None,
                    dimensions: Some(1024),
                    enable_thinking: None,
                    enable_cache: None,
                    rpm_limit: None,
                    tpm_limit: None,
                }));
            let embedding_for_config = embedding.unwrap_or(fallback_embedding);
            let attach_rag_components = |mut rag_config: RagConfig| {
                if let Some(p) = planner.clone() {
                    rag_config = rag_config.with_planner(p);
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
                if let Some(cache) = cache_store.clone() {
                    rag_config = rag_config.with_cache(cache);
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

        let quota_manager = pg
            .as_ref()
            .map(|p| Arc::new(avrag_billing::QuotaManager::new(p.clone())));
        let analytics = pg
            .as_ref()
            .map(|p| Arc::new(analytics::AnalyticsService::new(p.raw().clone())));
        let uses_memory_adapters = pg.is_none();
        let agent_service = Some(build_unified_agent_service(
            llm_client.clone(),
            config.agent_llm.temperature,
            search_executor.clone(),
            rag_runtime.clone(),
            &config.prompts.dir,
        ));
        let key_vault = build_key_vault_from_config(&config);

        Ok(Self {
            // config,  // REMOVED
            auth,
            pg,
            inner: Arc::new(RwLock::new(MemoryState::default())),
            llm_client,
            memory_llm_client,
            chatmemory,
            analytics,
            rag_runtime,
            agent_service,
            object_store,
            guard_pipeline: Arc::new(GuardPipeline::new()),
            quota_manager,
            uses_memory_adapters,
            // 提取非敏感配置
            public_base_url: config.public_base_url,
            object_root: config.object_root,
            usage_limit_phase: config.usage_limit.enforcement_phase,
            search_provider: config.search.provider,
            search_mode: config.search.mode,
            redis_url: config.redis.url.clone(),
            object_storage_upload_expire_sec: config.object_storage.upload_url_expire_sec,
            object_storage_download_expire_sec: config.object_storage.download_url_expire_sec,
            max_upload_file_size_bytes: config.max_upload_file_size_bytes,
            api_keys: Arc::new(RwLock::new(BTreeMap::new())),
            key_vault,
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

    /// Returns the runtime mode identifier ("postgres" or "memory").
    pub fn runtime_mode(&self) -> &'static str {
        if self.pg.is_some() {
            "postgres"
        } else {
            "memory"
        }
    }

    pub fn auth(&self) -> &AuthContext {
        &self.auth
    }

    pub fn with_auth(&self, auth: AuthContext) -> Self {
        let mut new_state = self.clone();
        new_state.auth = auth;
        new_state
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

    pub fn set_uses_memory_adapters(&mut self, value: bool) {
        self.uses_memory_adapters = value;
    }

    // 安全改造：提供辅助方法替代直接从 config 读取
    pub fn memory_llm_temperature(&self) -> Option<f32> {
        Some(0.2)
    }

    pub fn agent_llm_temperature(&self) -> Option<f32> {
        Some(0.2)
    }

    pub fn default_user_id(&self) -> String {
        // 返回默认用户 ID
        common::default_user_id()
    }

    pub fn redis_url(&self) -> &str {
        &self.redis_url
    }

    pub fn max_upload_file_size_bytes(&self) -> u64 {
        self.max_upload_file_size_bytes
    }

    pub fn key_vault(&self) -> Arc<dyn common::key_vault::KeyVault> {
        self.key_vault.clone()
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
                cm.load(&self.auth, session_uuid).await.ok()
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
        crate::agents::runtime::AgentRequest {
            kind,
            query: req.query.clone(),
            notebook_id,
            session_id,
            doc_scope,
            messages: req.messages.clone(),
            session_summary,
            user_preferences,
            debug: false,
            stream,
            language: req.language.clone(),
            auth_context: serde_json::to_value(&self.auth)
                .unwrap_or_else(|_| serde_json::json!({})),
            docscope_metadata: None,
            metadata: std::collections::BTreeMap::new(),
            cancellation_token: None,
            guard_pipeline: None,
            preferred_tools: vec![],
            format_hint: req.format_hint.clone(),
            max_iterations: None,
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
            serde_json::json!(
                agent_request.session_summary.is_some() || agent_request.user_preferences.is_some()
            ),
        );
        general_debug.insert("summary_updated".to_string(), serde_json::json!(false));
        general_debug.insert(
            "has_profile".to_string(),
            serde_json::json!(agent_request.user_preferences.is_some()),
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
}

pub struct CostEventRecord<'a> {
    pub event_name: analytics::CostEventName,
    pub feature: &'a str,
    pub session_id: Option<Uuid>,
    pub notebook_id: Option<Uuid>,
    pub usage: &'a avrag_llm::LlmUsage,
    pub source: &'a str,
    pub metadata: serde_json::Value,
}

impl AppState {
    pub async fn record_cost_event_if_available(&self, record: CostEventRecord<'_>) {
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
            session_id: record.session_id,
            notebook_id: record.notebook_id,
            event_name: record.event_name,
            feature: record.feature.to_string(),
            provider: non_empty_or_unknown(&record.usage.provider),
            model: non_empty_or_unknown(&record.usage.model),
            prompt_tokens: i64::from(record.usage.prompt_tokens),
            completion_tokens: i64::from(record.usage.completion_tokens),
            embedding_tokens: 0,
            usage_units: avrag_billing::usage_limit::compute_usage_units(
                &record.usage.provider,
                &record.usage.model,
                record.usage.prompt_tokens,
                record.usage.completion_tokens,
            ),
            storage_bytes_delta: 0,
            external_call_count: 0,
            source: record.source.to_string(),
            metadata: record.metadata,
        };
        if let Err(error) = analytics.record_cost_event(&event).await {
            telemetry::prometheus::record_dependency_failure("analytics");
            tracing::warn!(error = %error, event_name = ?record.event_name, "failed to record cost event");
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

pub(crate) fn non_empty_or_unknown(value: &str) -> String {
    if value.trim().is_empty() {
        "unknown".to_string()
    } else {
        value.to_string()
    }
}

fn agent_user_preferences_json(profile: &avrag_chatmemory::Layer3Profile) -> serde_json::Value {
    // If a structured profile exists, merge it with legacy fields for backward compatibility.
    let mut base = serde_json::json!({
        "expertise_domains": profile.expertise_domains.clone(),
        "preferred_answer_style": profile.preferred_answer_style.clone(),
        "frequently_asked_topics": profile.frequently_asked_topics.clone(),
        "custom_preferences": profile.custom_preferences.clone(),
        "inference_version": profile.inference_version.clone(),
    });
    if let (Some(base_obj), Some(profile_obj)) =
        (base.as_object_mut(), profile.structured_profile.as_object())
    {
        for (key, value) in profile_obj {
            // Structured profile fields take precedence over legacy fields.
            base_obj.insert(key.clone(), value.clone());
        }
    }
    base
}
