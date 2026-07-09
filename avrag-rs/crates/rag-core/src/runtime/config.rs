use std::sync::Arc;

use avrag_rag_core_ports::{
    CachePort, ChatPersistencePort, EmbeddingPort, PlannerPort, RerankPort,
};

use crate::ports::ContentStore;

/// Configuration for the RAG runtime (ports only — no avrag-llm concrete types).
#[derive(Clone)]
pub struct RagConfig {
    pub embedding_client: Arc<dyn EmbeddingPort>,
    pub mm_embedding_client: Option<Arc<dyn EmbeddingPort>>,
    /// Content store for sparse retrieval helpers and document lookups.
    pub content_store: Option<Arc<dyn ContentStore>>,
    /// Chat/session persistence for agent tools when not wired directly on the loop.
    pub chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
    /// Legacy retrieval planner for planner-compatible paths.
    pub planner: Option<Arc<dyn PlannerPort>>,
    /// Reranker for cross-encoder reranking
    pub reranker: Option<Arc<dyn RerankPort>>,
    /// Multimodal reranker for image/text candidates
    pub mm_reranker: Option<Arc<dyn RerankPort>>,
    /// Optional cache store for L2 retrieval and L4 generation caching.
    pub cache: Option<Arc<dyn CachePort>>,
}

impl RagConfig {
    pub fn new_for_data_plane(
        embedding_client: Arc<dyn EmbeddingPort>,
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
    pub fn with_planner(mut self, planner: Arc<dyn PlannerPort>) -> Self {
        self.planner = Some(planner);
        self
    }

    pub fn with_mm_embedding(mut self, embedding: Arc<dyn EmbeddingPort>) -> Self {
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
    pub fn with_reranker(mut self, reranker: Arc<dyn RerankPort>) -> Self {
        self.reranker = Some(reranker);
        self
    }

    pub fn with_mm_reranker(mut self, reranker: Arc<dyn RerankPort>) -> Self {
        self.mm_reranker = Some(reranker);
        self
    }

    /// Builder-style method to set the cache store
    pub fn with_cache(mut self, cache: Arc<dyn CachePort>) -> Self {
        self.cache = Some(cache);
        self
    }
}
