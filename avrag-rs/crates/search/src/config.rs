use std::sync::Arc;

use avrag_llm::{AnswerSynthesizer, LlmClient};

pub struct SearchConfig {
    pub mode: String,
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
    pub max_results: usize,
    pub max_sub_queries: usize,
    pub citation_required: bool,
    pub planner_enabled: bool,
    pub query_type_enabled: bool,
    pub extract_enabled: bool,
    pub planner_llm: Option<Arc<LlmClient>>,
    pub synthesizer: Option<Arc<AnswerSynthesizer>>,
    // Perplexity Agent API specific config
    pub perplexity_api_key: Option<String>,
    pub perplexity_model: String,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            mode: "llm_tools".to_string(),
            provider: "exa".to_string(),
            base_url: "https://api.exa.ai".to_string(),
            api_key: String::new(),
            max_results: 10,
            max_sub_queries: 3,
            citation_required: true,
            planner_enabled: true,
            query_type_enabled: true,
            extract_enabled: false,
            planner_llm: None,
            synthesizer: None,
            perplexity_api_key: None,
            perplexity_model: "nvidia/nemotron-3-super-120b-a12b".to_string(),
        }
    }
}
