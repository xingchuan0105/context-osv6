use std::sync::Arc;

use avrag_llm::{AnswerSynthesizer, EmbeddingClient, RerankerClient, RetrievalPlanner};
use avrag_storage_pg::PgAppRepository;

/// Configuration for the RAG runtime
#[derive(Clone)]
pub struct RagConfig {
    pub embedding_client: Arc<EmbeddingClient>,
    pub mm_embedding_client: Option<Arc<EmbeddingClient>>,
    /// PostgreSQL repository for sparse retrieval and content fetching.
    pub pg_repo: Option<Arc<PgAppRepository>>,
    /// Answer synthesizer for generating responses
    pub answer_synthesizer: Option<Arc<AnswerSynthesizer>>,
    /// Legacy retrieval planner for planner-compatible paths.
    pub planner: Option<Arc<RetrievalPlanner>>,
    /// Reranker for cross-encoder reranking
    pub reranker: Option<Arc<RerankerClient>>,
    /// Multimodal reranker for image/text candidates
    pub mm_reranker: Option<Arc<RerankerClient>>,
}

impl RagConfig {
    pub fn new_for_data_plane(
        embedding_client: Arc<EmbeddingClient>,
        pg_repo: Option<Arc<PgAppRepository>>,
    ) -> Self {
        Self {
            embedding_client,
            mm_embedding_client: None,
            pg_repo,
            answer_synthesizer: None,
            planner: None,
            reranker: None,
            mm_reranker: None,
        }
    }

    /// Builder-style method to set the answer synthesizer
    pub fn with_synthesizer(mut self, synthesizer: Arc<AnswerSynthesizer>) -> Self {
        self.answer_synthesizer = Some(synthesizer);
        self
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

    /// Builder-style method to set the reranker
    pub fn with_reranker(mut self, reranker: Arc<RerankerClient>) -> Self {
        self.reranker = Some(reranker);
        self
    }

    pub fn with_mm_reranker(mut self, reranker: Arc<RerankerClient>) -> Self {
        self.mm_reranker = Some(reranker);
        self
    }
}
