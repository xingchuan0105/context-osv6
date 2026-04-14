pub mod client;
pub mod embedding;
pub mod planner;
pub mod reranker;
pub mod summary;
pub mod synthesizer;

pub use client::{ChatMessage, LlmClient, LlmResponse, LlmUsage};
pub use embedding::{EmbeddingClient, MultiModalEmbeddingInput};
pub use planner::RetrievalPlanner;
pub use reranker::{
    MultiModalRerankDocument, MultiModalRerankResult, RerankResult, RerankerClient,
};
pub use summary::SummaryGenerator;
pub use synthesizer::{AnswerSynthesizer, SynthesisOutput};

/// API dispatch style — determines request/response format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiStyle {
    OpenAi,
    DashScopeMultimodalEmbedding,
    DashScopeVlRerank,
    Auto,
}

impl ApiStyle {
    /// Parse from the string values used in env vars and config.
    pub fn from_config_str(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "openai" => Some(Self::OpenAi),
            "dashscope_multimodal_embedding" => Some(Self::DashScopeMultimodalEmbedding),
            "dashscope_vl_rerank" => Some(Self::DashScopeVlRerank),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }
}

impl std::fmt::Display for ApiStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::OpenAi => "openai",
            Self::DashScopeMultimodalEmbedding => "dashscope_multimodal_embedding",
            Self::DashScopeVlRerank => "dashscope_vl_rerank",
            Self::Auto => "auto",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone)]
pub struct ModelProviderConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout_ms: u64,
    pub api_style: Option<ApiStyle>,
    pub dimensions: Option<usize>,
    pub enable_thinking: Option<bool>,
}

impl ModelProviderConfig {
    pub fn is_configured(&self) -> bool {
        !self.api_key.is_empty() && !self.base_url.is_empty()
    }

    pub fn provider_name(&self) -> String {
        let url = self.base_url.to_ascii_lowercase();
        if url.contains("dashscope") {
            "dashscope".to_string()
        } else if url.contains("openai") {
            "openai".to_string()
        } else if url.contains("siliconflow") {
            "siliconflow".to_string()
        } else if url.contains("perplexity") {
            "perplexity".to_string()
        } else {
            "unknown".to_string()
        }
    }
}
