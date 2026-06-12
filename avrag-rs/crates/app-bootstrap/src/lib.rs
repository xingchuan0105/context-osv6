mod adapters;
mod config_helpers;
mod domain_row_convert;
mod pg_error;

use adapters::{
    ObjectStorePortAdapter, PgAdminStoreAdapter, PgBillingQuotaAdapter, PgChatPersistenceAdapter,
    PgDocumentStoreAdapter, PgHealthAdapter,
};
use app_admin::AdminContext;
use app_billing::BillingContext;
use app_chat::{ChatContext, LlmContext, OrchestratorContext};
use app_core::{
    AdminStorePort, AnalyticsServiceCtx, AppConfig, BillingQuotaPort, ChatPersistencePort,
    DocumentStorePort, MemoryState, StorageContext,
};
use app_documents::DocumentContext;
use avrag_auth::AuthContext;
use avrag_chatmemory::ChatMemory;
use avrag_guardrails::GuardPipeline;
use avrag_llm::EmbeddingClient;
use avrag_rag_core::{
    RagConfig, RagRuntime, RetrievalDataPlane,
};
use avrag_search::SearchExecutor;
use avrag_storage_milvus::{MilvusConfig as StorageMilvusConfig, MilvusDataPlane};
use avrag_storage_pg::{ObjectStoreHandle, PgAppRepository};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

pub use config_helpers::{
    auth_context_from_config, build_object_store, build_unified_agent_service,
    make_embedding_client, make_llm_client, make_planner, make_reranker,
};

#[derive(Clone)]
pub struct AppBootstrapResult {
    pub auth: AuthContext,
    pub storage: StorageContext,
    pub llm_ctx: LlmContext,
    pub orchestrator: OrchestratorContext,
    pub analytics: AnalyticsServiceCtx,
    pub billing: BillingContext,
    pub admin: AdminContext,
    pub documents: DocumentContext,
    pub chat: ChatContext,
    pub postgres: Option<Arc<PgAppRepository>>,
    pub redis_url: String,
}

fn build_chat_context(
    auth: &AuthContext,
    storage: &StorageContext,
    llm_ctx: &LlmContext,
    orchestrator: &OrchestratorContext,
    analytics: &AnalyticsServiceCtx,
    billing: &BillingContext,
    admin: &AdminContext,
    documents: &DocumentContext,
) -> ChatContext {
    ChatContext::new(
        auth.clone(),
        storage.clone(),
        llm_ctx.clone(),
        orchestrator.clone(),
        analytics.clone(),
        billing.clone(),
        admin.clone(),
        documents.clone(),
    )
}

pub fn new_memory(config: AppConfig) -> AppBootstrapResult {
    let auth = auth_context_from_config(&config);
    let llm_ctx = LlmContext::new(
        make_llm_client(&config.agent_llm),
        make_llm_client(&config.memory_llm),
    );
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
        llm_ctx.agent_client().cloned(),
        search_executor.clone(),
        None,
        None,
        &config.prompts.dir,
    ));
    let object_store: Arc<dyn app_core::ObjectStorePort> = Arc::new(ObjectStorePortAdapter::new(
        Arc::new(ObjectStoreHandle::local(PathBuf::from(
            config.object_root.clone(),
        ))),
    ));
    let storage = StorageContext::new(
        None,
        false,
        None,
        None,
        None,
        None,
        Arc::new(RwLock::new(MemoryState::default())),
        Arc::new(RwLock::new(BTreeMap::new())),
        config.max_upload_file_size_bytes,
        true,
        object_store,
        config.public_base_url.clone(),
        config.object_root.clone(),
        config.object_storage.upload_url_expire_sec,
        config.object_storage.download_url_expire_sec,
    );
    let orchestrator = OrchestratorContext::new(
        agent_service,
        chatmemory,
        Arc::new(GuardPipeline::new()),
        None,
    );

    let billing = BillingContext::new(
        None,
        config.usage_limit.enforcement_phase.clone(),
    );
    let admin = AdminContext::new();
    let documents = DocumentContext::new();
    let analytics = AnalyticsServiceCtx::new(None);
    let chat = build_chat_context(
        &auth,
        &storage,
        &llm_ctx,
        &orchestrator,
        &analytics,
        &billing,
        &admin,
        &documents,
    );

    AppBootstrapResult {
        auth,
        storage,
        llm_ctx,
        orchestrator,
        analytics,
        billing,
        admin,
        documents,
        chat,
        postgres: None,
        redis_url: config.redis.url.clone(),
    }
}

pub async fn bootstrap(config: AppConfig) -> anyhow::Result<AppBootstrapResult> {
    let auth = auth_context_from_config(&config);
    let object_store_handle = Arc::new(build_object_store(&config).await?);
    let pg = if let Some(database_url) = config.database_url.as_deref() {
        let repository = PgAppRepository::connect(database_url).await?;
        if config.auto_migrate {
            repository.migrate().await?;
        }
        Some(Arc::new(repository))
    } else {
        None
    };

    let llm_ctx = LlmContext::new(
        make_llm_client(&config.agent_llm),
        make_llm_client(&config.memory_llm),
    );
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

    let cache_store = if config.redis.url.trim().is_empty() {
        None
    } else {
        avrag_cache_redis::CacheStore::new(&config.redis.url)
            .ok()
            .map(Arc::new)
    };

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
            Some(Arc::new(app_documents::PgContentStore::new(
                pg_repo.clone(),
            )) as Arc<dyn common::ContentStore>),
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
    let billing = BillingContext::new(
        quota_manager,
        config.usage_limit.enforcement_phase.clone(),
    );
    let analytics = AnalyticsServiceCtx::new(
        pg.as_ref()
            .map(|p| Arc::new(analytics::AnalyticsService::new(p.raw().clone()))),
    );
    let object_store: Arc<dyn app_core::ObjectStorePort> =
        Arc::new(ObjectStorePortAdapter::new(object_store_handle));
    let postgres_health = pg
        .as_ref()
        .map(|repo| Arc::new(PgHealthAdapter::new(repo.clone())) as Arc<dyn app_core::PostgresHealthPort>);
    let uses_memory_adapters = pg.is_none();
    let document_store: Option<Arc<dyn DocumentStorePort>> = pg.as_ref().map(|repository| {
        Arc::new(PgDocumentStoreAdapter::new(repository.clone())) as Arc<dyn DocumentStorePort>
    });
    let admin_store: Option<Arc<dyn AdminStorePort>> = pg.as_ref().map(|repository| {
        Arc::new(PgAdminStoreAdapter::new(repository.clone())) as Arc<dyn AdminStorePort>
    });
    let billing_quota: Option<Arc<dyn BillingQuotaPort>> =
        document_store.as_ref().map(|store| {
            Arc::new(PgBillingQuotaAdapter::new(billing.clone(), store.clone()))
                as Arc<dyn BillingQuotaPort>
        });
    let chat_persistence: Option<Arc<dyn ChatPersistencePort>> = pg
        .as_ref()
        .map(|repository| {
            Arc::new(PgChatPersistenceAdapter::new(repository.clone()))
                as Arc<dyn ChatPersistencePort>
        });
    let agent_service = Some(build_unified_agent_service(
        llm_ctx.agent_client().cloned(),
        search_executor.clone(),
        rag_runtime.clone(),
        chat_persistence.clone(),
        &config.prompts.dir,
    ));
    let storage = StorageContext::new(
        postgres_health,
        pg.is_some(),
        document_store,
        admin_store,
        billing_quota,
        chat_persistence,
        Arc::new(RwLock::new(MemoryState::default())),
        Arc::new(RwLock::new(BTreeMap::new())),
        config.max_upload_file_size_bytes,
        uses_memory_adapters,
        object_store,
        config.public_base_url.clone(),
        config.object_root.clone(),
        config.object_storage.upload_url_expire_sec,
        config.object_storage.download_url_expire_sec,
    );
    let orchestrator = OrchestratorContext::new(
        agent_service,
        chatmemory,
        Arc::new(GuardPipeline::new()),
        rag_runtime,
    );

    let admin = AdminContext::new();
    let documents = DocumentContext::new();
    let chat = build_chat_context(
        &auth,
        &storage,
        &llm_ctx,
        &orchestrator,
        &analytics,
        &billing,
        &admin,
        &documents,
    );

    Ok(AppBootstrapResult {
        auth,
        storage,
        llm_ctx,
        orchestrator,
        analytics,
        billing,
        admin,
        documents,
        chat,
        postgres: pg,
        redis_url: config.redis.url.clone(),
    })
}
