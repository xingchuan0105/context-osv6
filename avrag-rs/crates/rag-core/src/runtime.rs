mod config;
mod execute;
mod planner;
mod response;
mod response_utils;
mod retrieval;
pub mod bridge;
pub mod tools;

#[cfg(test)]
mod tests;

use std::sync::Arc;

pub use self::config::RagConfig;
pub use avrag_retrieval_data_plane::{RetrievalDataPlane, WeightedChunkList};

/// RAG runtime — retrieval, synthesis, and response-building helpers.
///
/// `avrag-app` owns the chat orchestration pipeline. This crate stays focused on
/// stage-level retrieval operations and tool dispatch for RAG.
use avrag_llm::EmbeddingClient;

pub struct RagRuntime {
    config: RagConfig,
    data_plane: Arc<dyn RetrievalDataPlane>,
    embedding_client: Option<Arc<EmbeddingClient>>,
}

impl RagRuntime {
    pub fn with_data_plane(config: RagConfig, data_plane: Arc<dyn RetrievalDataPlane>) -> Self {
        Self {
            config,
            data_plane,
            embedding_client: None,
        }
    }

    pub fn with_embedding_client(mut self, embedding_client: Arc<EmbeddingClient>) -> Self {
        self.embedding_client = Some(embedding_client);
        self
    }

    /// Access the cache store if configured.
    pub fn cache(&self) -> Option<&avrag_cache_redis::CacheStore> {
        self.config.cache.as_deref()
    }

    /// Clone the cache store Arc if configured.
    pub fn cache_arc(&self) -> Option<std::sync::Arc<avrag_cache_redis::CacheStore>> {
        self.config.cache.clone()
    }

    /// Access the PostgreSQL repository if configured.
    pub fn pg_repo(&self) -> Option<std::sync::Arc<avrag_storage_pg::PgAppRepository>> {
        self.config.pg_repo.clone()
    }

    /// Access the reranker client if configured.
    pub fn reranker(&self) -> Option<Arc<avrag_llm::RerankerClient>> {
        self.config.reranker.clone()
    }

    /// Execute a batch of tool calls in parallel and return their results.
    pub async fn execute_tools(
        &self,
        auth: &avrag_auth::AuthContext,
        calls: Vec<common::ToolCall>,
    ) -> Vec<common::ToolResult> {
        tools::dispatch_all(self, auth, calls).await
    }
}

const TOTAL_CANDIDATE_BUDGET: usize = 100;
const GLOBAL_RRF_K: usize = 60;
const FINAL_RERANK_BUDGET: usize = TOTAL_CANDIDATE_BUDGET;
const FINAL_MIN_CHUNKS: usize = 30;
const FINAL_SCORE_THRESHOLD: f32 = 0.7;
