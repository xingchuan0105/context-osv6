use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    /// 引用索引，Perplexity 返回的引用标记（如 [1], [2]）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citation_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub query_type: String,
    pub sub_queries: Vec<String>,
    pub results: Vec<SearchResult>,
    pub synthesized_answer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planner_usage: Option<avrag_llm::LlmUsage>,
}
