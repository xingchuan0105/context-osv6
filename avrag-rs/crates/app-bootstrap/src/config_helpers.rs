use app_chat::agents::service::UnifiedAgentService;
use anyhow::Result as AnyResult;
use app_core::{AppConfig, ChatPersistencePort, ModelProviderConfig};
use avrag_auth::{ActorId, AuthContext, OrgId, SubjectKind};
use avrag_llm::{EmbeddingClient, LlmClient, RerankerClient, RetrievalPlanner};
use avrag_rag_core::RagRuntime;
use avrag_search::SearchExecutor;
use avrag_storage_pg::{ObjectStoreHandle, S3ObjectStore};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

pub fn auth_context_from_config(config: &AppConfig) -> AuthContext {
    let org_uuid = Uuid::parse_str(&config.org_id).unwrap_or_else(|_| Uuid::nil());
    let user_uuid = Uuid::parse_str(&config.user_id).unwrap_or_else(|_| Uuid::nil());
    AuthContext::new(OrgId::from(org_uuid), SubjectKind::User)
        .with_actor_id(ActorId::new(user_uuid))
        .with_request_id("config-bootstrap")
}

pub fn make_llm_client(config: &ModelProviderConfig) -> Option<LlmClient> {
    config.to_llm_config().map(LlmClient::new)
}

pub fn build_unified_agent_service(
    llm_client: Option<LlmClient>,
    search_executor: Option<Arc<SearchExecutor>>,
    rag_runtime: Option<Arc<RagRuntime>>,
    chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
    _prompts_dir: &str,
) -> Arc<UnifiedAgentService> {
    let search_provider: Option<Arc<dyn avrag_search::SearchProvider>> =
        search_executor.map(|executor| -> Arc<dyn avrag_search::SearchProvider> { executor });

    let agent = app_chat::agents::unified::UnifiedAgent::new(llm_client.clone())
        .with_rag_runtime(rag_runtime)
        .with_search_executor(search_provider)
        .with_chat_persistence(chat_persistence);

    Arc::new(UnifiedAgentService::new(Box::new(agent)))
}

pub fn make_embedding_client(
    config: &ModelProviderConfig,
    cache: Option<Arc<avrag_cache_redis::CacheStore>>,
) -> Option<Arc<EmbeddingClient>> {
    config.to_llm_config().map(|c| {
        let client = EmbeddingClient::new(c);
        let client = if let Some(cache) = cache {
            client.with_cache(cache)
        } else {
            client
        };
        Arc::new(client)
    })
}

pub fn make_planner(
    config: &ModelProviderConfig,
    cache: Option<Arc<avrag_cache_redis::CacheStore>>,
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
