mod adapters;
mod app_state;
mod product_apps;

pub use product_apps::{
    AdminApp, AdminOpsApp, AgentApp, BillingApp, ConversationApp, PrefsApp, ShareApp,
    WorkspaceApp, WorkspaceApiKeyAuth,
};
mod config_helpers;
mod domain_row_convert;
mod pg_error;
mod services;

pub use app_state::{
    AppState, CostEventRecord, MemoryState, RetrievedContext, StoredDocument, agent_icon,
    agent_name, build_answer, build_citations, build_degrade_trace, build_docscope_metadata,
    build_mode_debug, build_parsed_preview, build_planner_output, build_redis_url, build_sources,
    build_summary, derive_profile_domains, derive_profile_topics, detect_preferred_style,
    document_is_deleting_or_deleted, estimate_token_count, infer_mime_type_from_path,
    is_remote_asset_reference, merge_general_profile_custom_preferences, next_message_id,
    status_label,
};

pub use adapters::{
    PgBillingStoreAdapter, PgUsageLimitStoreAdapter, RedisFixedWindowRateLimiter,
    RedisRateLimitBackend, build_rate_limit_backend,
};

use adapters::{
    ObjectStorePortAdapter, PgAdminStoreAdapter, PgAuthStoreAdapter, PgBillingQuotaAdapter,
    PgChatPersistenceAdapter, PgDocumentStoreAdapter, PgHealthAdapter, PgShareStoreAdapter,
};
use app_admin::AdminContext;
use app_billing::BillingContext;
use app_chat::{ChatContext, LlmContext, OrchestratorContext};
use app_core::{
    AdminStorePort, AnalyticsServiceCtx, AppConfig, AuthStorePort, BillingQuotaPort,
    BillingStorePort, ChatPersistencePort, DocumentStorePort, MemoryStateHandles,
    ObjectStoreConfig, ShareStorePort, StorageContext, StorageContextParts, StorageInfra,
    StorageStores,
};
use app_documents::DocumentContext;
use contracts::auth_runtime::AuthContext;
use avrag_chatmemory::ChatMemory;
use avrag_guardrails::GuardPipeline;
use avrag_rag_core::{RagConfig, RagRuntime, RetrievalDataPlane};
use avrag_search::SearchExecutor;
use avrag_storage_milvus::{MilvusConfig as StorageMilvusConfig, MilvusDataPlane};
use avrag_storage_pg::{BootstrapRepository, ObjectStoreHandle, PgAppRepository, TenantPgPool};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

pub use config_helpers::{
    auth_context_from_config, build_object_store, build_unified_agent_service,
    make_embedding_client, make_llm_client, make_planner, make_reranker,
};

pub use services::{
    PasswordResetConfig, PasswordResetError, PasswordResetService, SendResetCodeOutcome,
    VerifyResetCodeOutcome,
};

#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub mod test_support {
    pub use crate::adapters::{
        PgAdminStoreAdapter, PgChatPersistenceAdapter, PgDocumentStoreAdapter, PgHealthAdapter,
    };
}

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
    pub rate_limit_backend: Option<Arc<RedisRateLimitBackend>>,
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
        timeout_ms: config.search.timeout_ms,
        search_lang: config.search.search_lang.clone(),
        country: config.search.country.clone(),
        freshness: config.search.freshness.clone(),
    })));
    let object_store: Arc<dyn app_core::ObjectStorePort> =
        Arc::new(ObjectStorePortAdapter::new(Arc::new(
            ObjectStoreHandle::local(PathBuf::from(config.object_root.clone())),
        )));
    let memory_state = Arc::new(RwLock::new(MemoryState::default()));
    let api_keys = Arc::new(RwLock::new(BTreeMap::new()));
    let api_key_hashes = Arc::new(RwLock::new(BTreeMap::new()));
    let document_store: Option<Arc<dyn app_core::DocumentStorePort>> = Some(Arc::new(
        app_core::MemoryDocumentStore::new(memory_state.clone()),
    ));
    let chat_persistence: Option<Arc<dyn ChatPersistencePort>> = Some(Arc::new(
        app_core::MemoryChatPersistence::new(memory_state.clone()),
    ));
    let admin_store: Option<Arc<dyn app_core::AdminStorePort>> = Some(Arc::new(
        app_core::MemoryAdminStore::new(
            memory_state.clone(),
            api_keys.clone(),
            api_key_hashes.clone(),
        ),
    ));
    let billing_quota: Option<Arc<dyn app_core::BillingQuotaPort>> =
        Some(Arc::new(app_core::MemoryBillingQuotaPort));
    let agent_service = Some(build_unified_agent_service(
        llm_ctx.agent_client().cloned(),
        search_executor.clone(),
        None,
        chat_persistence.clone(),
        None,
        &config.prompts.dir,
    ));
    let storage = StorageContext::from_parts(StorageContextParts {
        infra: StorageInfra {
            postgres_health: None,
            postgres_configured: false,
            uses_memory_adapters: StorageInfra::memory_adapters_flag(true),
            max_upload_file_size_bytes: config.max_upload_file_size_bytes,
        },
        stores: StorageStores {
            document_store,
            auth_store: None,
            admin_store,
            billing_quota,
            billing_store: None,
            share_store: None,
            chat_persistence,
        },
        memory: MemoryStateHandles {
            inner: memory_state,
            api_keys,
            api_key_hashes,
        },
        objects: ObjectStoreConfig {
            object_store,
            public_base_url: config.public_base_url.clone(),
            object_root: config.object_root.clone(),
            upload_expire_sec: config.object_storage.upload_url_expire_sec,
            download_expire_sec: config.object_storage.download_url_expire_sec,
        },
    });
    let orchestrator = OrchestratorContext::new(
        agent_service,
        chatmemory,
        Arc::new(GuardPipeline::new()),
        None,
    );

    let billing = BillingContext::new(None, config.usage_limit.enforcement_phase.clone());
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
        rate_limit_backend: build_rate_limit_backend(&config.redis.url),
    }
}

pub async fn bootstrap(config: AppConfig) -> anyhow::Result<AppBootstrapResult> {
    let auth = auth_context_from_config(&config);
    let object_store_handle = Arc::new(build_object_store(&config).await?);
    let pg = if let Some(database_url) = config.database_url.as_deref() {
        let bootstrap = BootstrapRepository::connect(database_url).await?;
        if config.auto_migrate {
            bootstrap.migrate().await?;
        }
        let repository = PgAppRepository {
            pool: TenantPgPool::new(bootstrap.raw().clone()),
        };
        Some(Arc::new(repository))
    } else {
        None
    };

    let llm_ctx = LlmContext::new(
        make_llm_client(&config.agent_llm),
        make_llm_client(&config.memory_llm),
    );
    let chat_persistence_adapter = pg
        .as_ref()
        .map(|repository| Arc::new(PgChatPersistenceAdapter::new(repository.clone())));
    let chat_persistence: Option<Arc<dyn ChatPersistencePort>> = chat_persistence_adapter
        .as_ref()
        .map(|adapter| adapter.clone() as Arc<dyn ChatPersistencePort>);
    // ChatMemory only needs MessagePort + ProfilePort (ISP); coerce from concrete adapter.
    let chatmemory = chat_persistence_adapter.as_ref().map(|adapter| {
        let messages: Arc<dyn app_core::MessagePort> = adapter.clone();
        let profile: Arc<dyn app_core::ProfilePort> = adapter.clone();
        Arc::new(ChatMemory::new(messages, profile))
    });
    let search_executor = Some(Arc::new(SearchExecutor::new(avrag_search::SearchConfig {
        provider: config.search.provider.clone(),
        base_url: config.search.base_url.clone(),
        api_key: config.search.api_key.clone(),
        max_results: config.search.max_results,
        timeout_ms: config.search.timeout_ms,
        search_lang: config.search.search_lang.clone(),
        country: config.search.country.clone(),
        freshness: config.search.freshness.clone(),
    })));

    let cache_store: Option<Arc<dyn avrag_rag_core_ports::CachePort>> =
        if config.redis.url.trim().is_empty() {
            None
        } else {
            avrag_cache_redis::CacheStore::new(&config.redis.url)
                .ok()
                .map(|store| Arc::new(store) as Arc<dyn avrag_rag_core_ports::CachePort>)
        };

    let billing_store: Option<Arc<dyn BillingStorePort>> = pg.as_ref().map(|repository| {
        Arc::new(PgBillingStoreAdapter::new(repository.clone())) as Arc<dyn BillingStorePort>
    });
    let share_store: Option<Arc<dyn ShareStorePort>> = pg.as_ref().map(|repository| {
        Arc::new(PgShareStoreAdapter::new(repository.clone())) as Arc<dyn ShareStorePort>
    });
    let usage_limit_store = pg.as_ref().map(|repository| {
        Arc::new(PgUsageLimitStoreAdapter::new(repository.clone()))
            as Arc<dyn app_core::UsageLimitStorePort>
    });

    let usage_observer: Option<Arc<dyn avrag_llm::UsageObserver>> =
        usage_limit_store.as_ref().map(|store| {
            Arc::new(app_billing::PgUsageObserver::new(store.clone()))
                as Arc<dyn avrag_llm::UsageObserver>
        });
    // Embeddings use bootstrap org/user (system-ish); chat agent rebinds tenant per request.
    let embedding_tenant = {
        let auth = auth_context_from_config(&config);
        avrag_llm::TenantContext {
            org_id: auth.org_id().into_uuid(),
            user_id: auth
                .actor_id()
                .map(|a| a.into_uuid())
                .unwrap_or_else(uuid::Uuid::nil),
        }
    };
    let embedding_observer = usage_observer
        .as_ref()
        .map(|obs| (obs.clone(), embedding_tenant.clone()));

    let quota_manager =
        billing_store
            .as_ref()
            .zip(usage_limit_store.as_ref())
            .map(|(billing, usage_limit)| {
                Arc::new(avrag_billing::QuotaManager::new(
                    billing.clone(),
                    usage_limit.clone(),
                ))
            });

    let rag_runtime = if config.enable_rag && pg.is_some() {
        let pg_repo = pg.as_ref().unwrap();
        let embedding = make_embedding_client(
            &config.embedding,
            cache_store.clone(),
            embedding_observer.clone(),
        )
            .ok_or_else(|| anyhow::anyhow!("embedding client is required when enable_rag=true"))?;
        let mm_embedding = make_embedding_client(
            &config.mm_embedding,
            cache_store.clone(),
            embedding_observer.clone(),
        );
        let planner = make_planner(&config.agent_llm, cache_store.clone());
        let reranker = make_reranker(&config.rerank);
        let mm_reranker = make_reranker(&config.mm_rerank);

        let attach_rag_components = |mut rag_config: RagConfig| {
            if let Some(p) = planner.clone() {
                rag_config = rag_config
                    .with_planner(p as Arc<dyn avrag_rag_core_ports::PlannerPort>);
            }
            if let Some(mm) = mm_embedding.clone() {
                rag_config = rag_config
                    .with_mm_embedding(mm as Arc<dyn avrag_rag_core_ports::EmbeddingPort>);
            }
            if let Some(r) = reranker.clone() {
                rag_config =
                    rag_config.with_reranker(r as Arc<dyn avrag_rag_core_ports::RerankPort>);
            }
            if let Some(mm_r) = mm_reranker.clone() {
                rag_config =
                    rag_config.with_mm_reranker(mm_r as Arc<dyn avrag_rag_core_ports::RerankPort>);
            }
            if let Some(cache) = cache_store.clone() {
                rag_config = rag_config.with_cache(cache);
            }
            rag_config.with_chat_persistence(chat_persistence.clone())
        };

        let rag_config = attach_rag_components(RagConfig::new_for_data_plane(
            embedding as Arc<dyn avrag_rag_core_ports::EmbeddingPort>,
            Some(Arc::new(avrag_storage_pg::PgContentStore::new(pg_repo.clone()))
                as Arc<dyn common::ContentStore>),
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
        let data_plane = Arc::new(MilvusDataPlane::new(milvus_config));
        data_plane.ensure_schema().await?;
        Some(Arc::new(RagRuntime::with_data_plane(
            rag_config,
            data_plane,
        )))
    } else {
        None
    };

    let mut billing = BillingContext::new(quota_manager, config.usage_limit.enforcement_phase.clone());
    if let Some(obs) = usage_observer.clone() {
        billing = billing.with_usage_observer(obs);
    }
    let analytics = AnalyticsServiceCtx::new(
        pg.as_ref()
            .map(|p| Arc::new(analytics::AnalyticsService::new(p.raw().clone()))),
    );
    let object_store: Arc<dyn app_core::ObjectStorePort> =
        Arc::new(ObjectStorePortAdapter::new(object_store_handle));
    let postgres_health = pg.as_ref().map(|repo| {
        Arc::new(PgHealthAdapter::new(repo.clone())) as Arc<dyn app_core::PostgresHealthPort>
    });
    let uses_memory_adapters = pg.is_none();
    // Always install a document store: PG adapter when available, otherwise memory.
    // Domain code must not dual-path on Option — memory is an adapter, not a control-flow mode.
    let memory_state = Arc::new(RwLock::new(MemoryState::default()));
    let api_keys = Arc::new(RwLock::new(BTreeMap::new()));
    let api_key_hashes = Arc::new(RwLock::new(BTreeMap::new()));
    let document_store: Option<Arc<dyn DocumentStorePort>> = Some(match pg.as_ref() {
        Some(repository) => {
            Arc::new(PgDocumentStoreAdapter::new(repository.clone())) as Arc<dyn DocumentStorePort>
        }
        None => Arc::new(app_core::MemoryDocumentStore::new(memory_state.clone()))
            as Arc<dyn DocumentStorePort>,
    });
    // Always install an admin store (PG or memory) so domain code never dual-paths.
    let admin_store: Option<Arc<dyn AdminStorePort>> = Some(match pg.as_ref() {
        Some(repository) => {
            Arc::new(PgAdminStoreAdapter::new(repository.clone())) as Arc<dyn AdminStorePort>
        }
        None => Arc::new(app_core::MemoryAdminStore::new(
            memory_state.clone(),
            api_keys.clone(),
            api_key_hashes.clone(),
        )) as Arc<dyn AdminStorePort>,
    });
    let auth_store: Option<Arc<dyn AuthStorePort>> = pg.as_ref().map(|repository| {
        Arc::new(PgAuthStoreAdapter::new(repository.clone())) as Arc<dyn AuthStorePort>
    });
    let billing_quota: Option<Arc<dyn BillingQuotaPort>> = match (pg.as_ref(), document_store.as_ref()) {
        (Some(_), Some(store)) => Some(Arc::new(PgBillingQuotaAdapter::new(
            billing.clone(),
            store.clone(),
        )) as Arc<dyn BillingQuotaPort>),
        (None, _) => Some(Arc::new(app_core::MemoryBillingQuotaPort) as Arc<dyn BillingQuotaPort>),
        _ => None,
    };
    let agent_service = Some(build_unified_agent_service(
        llm_ctx.agent_client().cloned(),
        search_executor.clone(),
        rag_runtime.clone(),
        chat_persistence.clone(),
        usage_observer.clone(),
        &config.prompts.dir,
    ));
    let storage = StorageContext::from_parts(StorageContextParts {
        infra: StorageInfra {
            postgres_health,
            postgres_configured: pg.is_some(),
            uses_memory_adapters: StorageInfra::memory_adapters_flag(uses_memory_adapters),
            max_upload_file_size_bytes: config.max_upload_file_size_bytes,
        },
        stores: StorageStores {
            document_store,
            auth_store,
            admin_store,
            billing_quota,
            billing_store,
            share_store,
            chat_persistence,
        },
        memory: MemoryStateHandles {
            inner: memory_state,
            api_keys,
            api_key_hashes,
        },
        objects: ObjectStoreConfig {
            object_store,
            public_base_url: config.public_base_url.clone(),
            object_root: config.object_root.clone(),
            upload_expire_sec: config.object_storage.upload_url_expire_sec,
            download_expire_sec: config.object_storage.download_url_expire_sec,
        },
    });
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
        rate_limit_backend: build_rate_limit_backend(&config.redis.url),
    })
}

#[cfg(feature = "test-support")]
mod app_state_test_support {
    use super::AppState;
    use app_core::StorageContext;
    use avrag_storage_pg::PgAppRepository;
    use std::sync::Arc;

    impl AppState {
        pub fn test_storage(&self) -> &StorageContext {
            &self.storage
        }

        pub fn test_set_postgres(&mut self, postgres: Arc<PgAppRepository>) {
            self.postgres = Some(postgres);
        }

        pub fn test_replace_storage(&mut self, storage: StorageContext) {
            self.storage = storage.clone();
            self.chat.storage = storage;
        }
    }
}
