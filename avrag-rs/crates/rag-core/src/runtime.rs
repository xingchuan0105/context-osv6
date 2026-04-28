mod config;
mod execute;
mod planner;
mod response;
mod response_utils;
mod retrieval;

#[cfg(test)]
mod tests;

use std::sync::Arc;

pub use self::config::RagConfig;
pub use avrag_retrieval_data_plane::{RetrievalDataPlane, WeightedChunkList};

/// RAG runtime used by GraphFlow.
///
/// `avrag-app` owns the chat orchestration graph. This crate stays focused on
/// stage-level retrieval, synthesis, and response-building helpers for RAG.
pub struct RagRuntime {
    config: RagConfig,
    data_plane: Arc<dyn RetrievalDataPlane>,
}

impl RagRuntime {
    pub fn with_data_plane(config: RagConfig, data_plane: Arc<dyn RetrievalDataPlane>) -> Self {
        Self { config, data_plane }
    }
}

const TOTAL_CANDIDATE_BUDGET: usize = 100;
const GLOBAL_RRF_K: usize = 60;
const FINAL_RERANK_BUDGET: usize = TOTAL_CANDIDATE_BUDGET;
const FINAL_MIN_CHUNKS: usize = 30;
const FINAL_SCORE_THRESHOLD: f32 = 0.7;
