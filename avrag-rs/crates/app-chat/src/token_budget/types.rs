//! Token budget simulation data model.
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct StageEstimate {
    pub stage: String,
    pub iteration: u8,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SimulationResult {
    pub scenario_name: String,
    pub mode: String,
    pub total_prompt_tokens: usize,
    pub total_completion_tokens: usize,
    pub total_tokens: usize,
    pub stages: Vec<StageEstimate>,
}

#[derive(Debug, Clone)]
pub struct Scenario {
    pub name: &'static str,
    pub mode: &'static str,
    pub query: &'static str,
    pub history: Vec<(&'static str, &'static str)>,
    pub user_preferences: Option<serde_json::Value>,
    /// Simulated search results (title + snippet).
    pub search_results: Vec<(&'static str, &'static str)>,
    /// Simulated RAG chunks (text).
    pub rag_chunks: Vec<&'static str>,
}
