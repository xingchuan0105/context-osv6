use anyhow::Result as AnyResult;
use app_chat::agents::service::UnifiedAgentService;
use app_core::{AppConfig, ChatPersistencePort, ModelProviderConfig};
use avrag_llm::{
    EmbeddingClient, LlmClient, RerankerClient, RetrievalPlanner, TenantContext, UsageObserver,
};
use avrag_rag_core::RagRuntime;
use avrag_search::SearchExecutor;
use avrag_storage_pg::{ObjectStoreHandle, S3ObjectStore};
use contracts::auth_runtime::{ActorId, AuthContext, SubjectKind, UserId};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

pub fn auth_context_from_config(config: &AppConfig) -> AuthContext {
    let org_uuid = Uuid::parse_str(&config.owner_user_id).unwrap_or_else(|_| Uuid::nil());
    let user_uuid = Uuid::parse_str(&config.user_id).unwrap_or_else(|_| Uuid::nil());
    AuthContext::new(UserId::from(org_uuid), SubjectKind::User)
        .with_actor_id(ActorId::new(user_uuid))
        .with_request_id("config-bootstrap")
}

pub fn make_llm_client(
    config: &ModelProviderConfig,
    pool: Option<avrag_llm::LlmPoolConfig>,
) -> Option<LlmClient> {
    config.to_llm_config().map(|llm_config| {
        let pool = pool.unwrap_or_else(|| avrag_llm::LlmPoolConfig::new(Vec::new()));
        LlmClient::new_with_pool(llm_config, pool)
    })
}

pub fn build_unified_agent_service(
    llm_client: Option<LlmClient>,
    retrieve_llm_client: Option<LlmClient>,
    search_executor: Option<Arc<SearchExecutor>>,
    rag_runtime: Option<Arc<RagRuntime>>,
    chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
    usage_observer: Option<Arc<dyn UsageObserver>>,
    _prompts_dir: &str,
) -> Arc<UnifiedAgentService> {
    build_unified_agent_service_with_secrets(
        llm_client,
        retrieve_llm_client,
        search_executor,
        rag_runtime,
        chat_persistence,
        usage_observer,
        None,
        _prompts_dir,
    )
}

pub fn build_unified_agent_service_with_secrets(
    llm_client: Option<LlmClient>,
    retrieve_llm_client: Option<LlmClient>,
    search_executor: Option<Arc<SearchExecutor>>,
    rag_runtime: Option<Arc<RagRuntime>>,
    chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
    usage_observer: Option<Arc<dyn UsageObserver>>,
    provider_secrets: Option<Arc<dyn app_core::ProviderSecretStorePort>>,
    _prompts_dir: &str,
) -> Arc<UnifiedAgentService> {
    let search_provider: Option<Arc<dyn avrag_search::SearchProvider>> =
        search_executor.map(|executor| -> Arc<dyn avrag_search::SearchProvider> { executor });

    let mut agent = app_chat::agents::unified::UnifiedAgent::new(llm_client.clone(), None, None)
        .with_retrieve_llm_client(retrieve_llm_client)
        .with_rag_runtime(rag_runtime)
        .with_search_executor(search_provider)
        .with_chat_persistence(chat_persistence)
        .with_provider_secrets(provider_secrets);
    if let Some(observer) = usage_observer {
        agent = agent.with_usage_observer(observer);
    }

    Arc::new(UnifiedAgentService::new(Box::new(agent)))
}

pub fn make_embedding_client(
    config: &ModelProviderConfig,
    cache: Option<Arc<dyn avrag_rag_core_ports::CachePort>>,
    usage_observer: Option<(Arc<dyn UsageObserver>, TenantContext)>,
    rate_gate: Option<Arc<dyn avrag_llm::EmbedRateGate>>,
) -> Option<Arc<EmbeddingClient>> {
    config.to_llm_config().map(|c| {
        let mut client = EmbeddingClient::new(c);
        if let Some(cache) = cache {
            client = client.with_cache(cache);
        }
        if let Some(gate) = rate_gate {
            client = client.with_rate_gate(gate);
        }
        if let Some((observer, tenant)) = usage_observer {
            client = client.with_observer(observer, tenant);
        }
        Arc::new(client)
    })
}

pub fn make_planner(
    config: &ModelProviderConfig,
    cache: Option<Arc<dyn avrag_rag_core_ports::CachePort>>,
) -> Option<Arc<RetrievalPlanner>> {
    config.to_llm_config().map(|c| {
        let planner = RetrievalPlanner::new(c);
        let planner = if let Some(cache) = cache {
            planner.with_cache(cache)
        } else {
            planner
        };
        Arc::new(planner)
    })
}

pub fn make_reranker(config: &ModelProviderConfig) -> Option<Arc<RerankerClient>> {
    config
        .to_llm_config()
        .map(|c| Arc::new(RerankerClient::new(c)))
}

/// Resolve the local/desktop user's active secret for a purpose at bootstrap.
/// `workspace_id = None` → account-default scope. Fail-open (None) on error.
pub async fn resolve_bootstrap_secret(
    store: &Option<Arc<dyn app_core::ProviderSecretStorePort>>,
    owner: Uuid,
    purpose: app_core::ProviderSecretPurpose,
) -> Option<app_core::ResolvedProviderSecret> {
    let store = store.as_ref()?;
    match store.resolve(owner, None, purpose).await {
        Ok(secret) => secret,
        Err(e) => {
            tracing::warn!(error = %e, purpose = purpose.as_str(), "bootstrap secret resolve failed");
            None
        }
    }
}

/// Build an embedding client from a resolved BYOK secret (G4) when no platform key.
pub fn embedding_client_from_secret(
    secret: Option<&app_core::ResolvedProviderSecret>,
    cache: Option<Arc<dyn avrag_rag_core_ports::CachePort>>,
    observer: Option<(Arc<dyn UsageObserver>, TenantContext)>,
    rate_gate: Option<Arc<dyn avrag_llm::EmbedRateGate>>,
) -> Option<Arc<EmbeddingClient>> {
    let cfg = secret?.to_llm_config()?;
    let mut client = EmbeddingClient::new(cfg);
    if let Some(cache) = cache {
        client = client.with_cache(cache);
    }
    if let Some(gate) = rate_gate {
        client = client.with_rate_gate(gate);
    }
    if let Some((obs, tenant)) = observer {
        client = client.with_observer(obs, tenant);
    }
    Some(Arc::new(client))
}

/// Build a reranker client from a resolved BYOK secret (G4) when no platform key.
pub fn reranker_from_secret(
    secret: Option<&app_core::ResolvedProviderSecret>,
) -> Option<Arc<RerankerClient>> {
    let cfg = secret?.to_llm_config()?;
    Some(Arc::new(RerankerClient::new(cfg)))
}

pub async fn build_object_store(config: &AppConfig) -> AnyResult<ObjectStoreHandle> {
    if !config.object_storage.endpoint.trim().is_empty()
        && !config.object_storage.bucket.trim().is_empty()
        && !config.object_storage.access_key.trim().is_empty()
        && !config.object_storage.secret_key.trim().is_empty()
    {
        let store = S3ObjectStore::new(
            config.object_storage.endpoint.clone(),
            config.object_storage.bucket.clone(),
            config.object_storage.region.clone(),
            config.object_storage.access_key.clone(),
            config.object_storage.secret_key.clone(),
            config.object_storage.use_path_style,
        )
        .await?;
        return Ok(ObjectStoreHandle::S3(store));
    }
    Ok(ObjectStoreHandle::local(PathBuf::from(
        config.object_root.clone(),
    )))
}
