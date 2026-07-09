use std::sync::Arc;

use avrag_llm::{EmbeddingClient, RerankerClient, RetrievalPlanner};
use avrag_rag_core_ports::ChatPersistencePort;

use crate::ports::ContentStore;

/// Configuration for the RAG runtime
#[derive(Clone)]
pub struct RagConfig {
    pub embedding_client: Arc<EmbeddingClient>,
    pub mm_embedding_client: Option<Arc<EmbeddingClient>>,
    /// Content store for sparse retrieval helpers and document lookups.
    pub content_store: Option<Arc<dyn ContentStore>>,
    /// Chat/session persistence for agent tools when not wired directly on the loop.
    pub chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
    /// Legacy retrieval planner for planner-compatible paths.
    pub planner: Option<Arc<RetrievalPlanner>>,
    /// Reranker for cross-encoder reranking
    pub reranker: Option<Arc<RerankerClient>>,
    /// Multimodal reranker for image/text candidates
    pub mm_reranker: Option<Arc<RerankerClient>>,
    /// Optional Redis cache store for L2 retrieval and L4 generation caching.
    pub cache: Option<std::sync::Arc<avrag_cache_redis::CacheStore>>,
}

impl RagConfig {
    pub fn new_for_data_plane(
        embedding_client: Arc<EmbeddingClient>,
        content_store: Option<Arc<dyn ContentStore>>,
    ) -> Self {
        Self {
            embedding_client,
            mm_embedding_client: None,
            content_store,
            chat_persistence: None,
            planner: None,
            reranker: None,
            mm_reranker: None,
            cache: None,
        }
    }

    /// Builder-style method to set the planner
    pub fn with_planner(mut self, planner: Arc<RetrievalPlanner>) -> Self {
        self.planner = Some(planner);
        self
    }

    pub fn with_mm_embedding(mut self, embedding: Arc<EmbeddingClient>) -> Self {
        self.mm_embedding_client = Some(embedding);
        self
    }

    pub fn with_chat_persistence(
        mut self,
        chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
    ) -> Self {
        self.chat_persistence = chat_persistence;
        self
    }

    /// Builder-style method to set the reranker
    pub fn with_reranker(mut self, reranker: Arc<RerankerClient>) -> Self {
        self.reranker = Some(reranker);
        self
    }

    pub fn with_mm_reranker(mut self, reranker: Arc<RerankerClient>) -> Self {
        self.mm_reranker = Some(reranker);
        self
    }

    /// Builder-style method to set the cache store
    pub fn with_cache(mut self, cache: Arc<avrag_cache_redis::CacheStore>) -> Self {
        self.cache = Some(cache);
        self
    }
}
