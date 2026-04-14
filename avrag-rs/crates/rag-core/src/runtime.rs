mod config;
mod planner;
mod retrieval;
mod response;
mod response_utils;

#[cfg(test)]
mod tests;

pub use self::config::RagConfig;
pub use self::config::WeightedChunkList;

/// RAG runtime used by GraphFlow.
///
/// `avrag-app` owns the chat orchestration graph. This crate stays focused on
/// stage-level retrieval, synthesis, and response-building helpers for RAG.
pub struct RagRuntime {
    config: RagConfig,
}

impl RagRuntime {
    pub fn new(config: RagConfig) -> Self {
        Self { config }
    }
}

const TOTAL_CANDIDATE_BUDGET: usize = 100;
const GLOBAL_RRF_K: usize = 60;
const FINAL_RERANK_BUDGET: usize = TOTAL_CANDIDATE_BUDGET;
const FINAL_MIN_CHUNKS: usize = 30;
const FINAL_SCORE_THRESHOLD: f32 = 0.7;
