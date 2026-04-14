use std::sync::Arc;

use avrag_llm::{AnswerSynthesizer, EmbeddingClient, RerankerClient, RetrievalPlanner};
use avrag_storage_pg::PgAppRepository;
use avrag_storage_qdrant::HttpQdrantBackend;
use serde::{Deserialize, Serialize};

use crate::retrieval::ScoredChunk;

/// Configuration for the RAG runtime
pub struct RagConfig {
    pub embedding_client: Arc<EmbeddingClient>,
    pub mm_embedding_client: Option<Arc<EmbeddingClient>>,
    pub qdrant_collection: String,
    /// Qdrant backend for dense retrieval
    pub qdrant: Arc<HttpQdrantBackend>,
    /// PostgreSQL repository for sparse retrieval and content fetching.
    pub pg_repo: Option<Arc<PgAppRepository>>,
    /// Answer synthesizer for generating responses
    pub answer_synthesizer: Option<Arc<AnswerSynthesizer>>,
    /// Retrieval planner for intent classification
    pub planner: Option<Arc<RetrievalPlanner>>,
    /// Reranker for cross-encoder reranking
    pub reranker: Option<Arc<RerankerClient>>,
    /// Multimodal reranker for image/text candidates
    pub mm_reranker: Option<Arc<RerankerClient>>,
}

impl RagConfig {
    pub fn new(
        embedding_client: Arc<EmbeddingClient>,
        qdrant: Arc<HttpQdrantBackend>,
        pg_repo: Option<Arc<PgAppRepository>>,
    ) -> Self {
        Self {
            embedding_client,
            mm_embedding_client: None,
            qdrant_collection: "chunks".to_string(),
            qdrant,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightedChunkList {
    pub weight: f32,
    pub chunks: Vec<ScoredChunk>,
}

