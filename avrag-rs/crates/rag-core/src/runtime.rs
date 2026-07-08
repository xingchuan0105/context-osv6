pub mod bridge;
mod config;
mod execute;
mod planner;
mod response;
mod response_utils;
mod retrieval;
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

#[derive(Clone)]
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

    // TODO(write-mode): WIP stub — attaches tenant identity for metering.
    pub fn with_tenant(self, _tenant: avrag_llm::TenantContext) -> Self {
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

    /// Access the content store if configured.
    pub fn content_store(&self) -> Option<Arc<dyn crate::ports::ContentStore>> {
        self.config.content_store.clone()
    }

    /// Access chat persistence if configured on the runtime.
    pub fn chat_persistence(&self) -> Option<Arc<dyn avrag_rag_core_ports::ChatPersistencePort>> {
        self.config.chat_persistence.clone()
    }

    /// Access the reranker client if configured.
    pub fn reranker(&self) -> Option<Arc<avrag_llm::RerankerClient>> {
        self.config.reranker.clone()
    }

    /// Execute a batch of tool calls in parallel and return their results.
    pub async fn execute_tools(
        &self,
        auth: &avrag_auth::AuthContext,
        calls: Vec<contracts::ToolCall>,
    ) -> Vec<contracts::ToolResult> {
        tools::dispatch_all(self, auth, calls).await
    }

    /// Count indexed text (body) chunks for a doc scope, for dynamic rough-recall sizing.
    pub async fn count_text_chunks(
        &self,
        auth: &avrag_auth::AuthContext,
        doc_ids: &[uuid::Uuid],
    ) -> anyhow::Result<usize> {
        self.data_plane.count_text_chunks(auth, doc_ids).await
    }

    /// List all text (body) chunks for a doc scope with full content, for the
    /// `doc_chunks` agent tool (codegen sandbox runs arbitrary traversal over
    /// the full set). See `doc_scan_design.md`.
    pub async fn list_text_chunks(
        &self,
        auth: &avrag_auth::AuthContext,
        doc_ids: &[uuid::Uuid],
    ) -> anyhow::Result<Vec<avrag_retrieval_data_plane::ScoredChunk>> {
        self.data_plane.list_text_chunks(auth, doc_ids).await
    }
}

const TOTAL_CANDIDATE_BUDGET: usize = 100;
const GLOBAL_RRF_K: usize = 60;
const FINAL_RERANK_BUDGET: usize = TOTAL_CANDIDATE_BUDGET;
const FINAL_MIN_CHUNKS: usize = 30;
const FINAL_SCORE_THRESHOLD: f32 = 0.7;

// Dynamic rough-recall sizing for agent-driven dense retrieval (dense_search tool).
// rough = clamp(docscope_chunk_total × ROUGH_RECALL_FRACTION, ROUGH_RECALL_MIN, ROUGH_RECALL_MAX)
// final = clamp(rough × FINAL_FEED_FRACTION, FINAL_FEED_MIN, FINAL_FEED_MAX)
const ROUGH_RECALL_FRACTION: f64 = 0.3;
const ROUGH_RECALL_MIN: usize = 50;
const ROUGH_RECALL_MAX: usize = 200;
const FINAL_FEED_FRACTION: f64 = 0.3;
const FINAL_FEED_MIN: usize = 10;
const FINAL_FEED_MAX: usize = 30;

/// Dynamic rough-recall budget from docscope chunk total.
/// Floors at `ROUGH_RECALL_MIN` so a zero/unknown count still yields a usable pool.
pub(crate) fn dynamic_rough_recall(chunk_total: usize) -> usize {
    let scaled = (chunk_total as f64 * ROUGH_RECALL_FRACTION).round() as usize;
    scaled.clamp(ROUGH_RECALL_MIN, ROUGH_RECALL_MAX)
}

/// Final chunks fed to the LLM from the reranked pool.
pub(crate) fn dynamic_final_feed(rough: usize) -> usize {
    let scaled = (rough as f64 * FINAL_FEED_FRACTION).round() as usize;
    scaled.clamp(FINAL_FEED_MIN, FINAL_FEED_MAX)
}

#[cfg(test)]
mod dynamic_budget_tests {
    use super::{dynamic_final_feed, dynamic_rough_recall};

    #[test]
    fn rough_recall_floors_when_chunk_total_is_small() {
        assert_eq!(dynamic_rough_recall(0), 50);
        assert_eq!(dynamic_rough_recall(10), 50);
        assert_eq!(dynamic_rough_recall(166), 50);
    }

    #[test]
    fn rough_recall_scales_at_thirty_percent_in_mid_range() {
        assert_eq!(dynamic_rough_recall(243), 73);
        assert_eq!(dynamic_rough_recall(300), 90);
        assert_eq!(dynamic_rough_recall(666), 200);
    }

    #[test]
    fn rough_recall_caps_at_max() {
        assert_eq!(dynamic_rough_recall(1000), 200);
        assert_eq!(dynamic_rough_recall(100_000), 200);
    }

    #[test]
    fn final_feed_scales_at_thirty_percent_of_rough() {
        assert_eq!(dynamic_final_feed(73), 22);
        assert_eq!(dynamic_final_feed(50), 15);
        assert_eq!(dynamic_final_feed(200), 30);
    }

    #[test]
    fn final_feed_floors_at_min() {
        assert_eq!(dynamic_final_feed(0), 10);
        assert_eq!(dynamic_final_feed(20), 10);
    }
}
