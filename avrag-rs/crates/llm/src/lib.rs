pub mod client;
pub mod completion_cache;
pub mod embedding;
pub mod planner;
pub mod protocols;
pub mod provider_profiles;
pub mod rate_limiter;
pub mod reranker;
pub mod route;
pub mod routing;
pub mod schema;
pub mod section_index;
pub mod summary;
pub mod synthesizer;
pub mod token_counter;
pub mod usage_observer;

pub use client::{ChatMessage, LlmClient, LlmResponse, LlmUsage};
pub use completion_cache::{CachedCompletion, CompletionCache};
pub use embedding::{EmbeddingClient, MultiModalEmbeddingInput};
pub use planner::RetrievalPlanner;
pub use protocols::{
    AnthropicMessagesProtocol, GeminiProtocol, OpenAiChatProtocol, OpenAiResponsesProtocol,
    Protocol,
};
pub use provider_profiles::{
    AuthStyle, PROVIDER_PROFILES, ProtocolKind, ProviderProfile, api_key_url_for_provider,
    find_provider_profile,
};
pub use rate_limiter::{
    RateLimitError, RateLimiter, SharedRateLimiter, default_rpm_limit, default_tpm_limit,
    provider_defaults,
};
pub use reranker::{
    MultiModalRerankDocument, MultiModalRerankResult, RerankResult, RerankerClient,
};
pub use route::{
    AnyRoute, Auth, DetectedProtocol, Endpoint, ReqwestTransport, Route, Transport, TransportBody,
    build_openai_chat_route, build_openai_responses_route, build_route_from_config,
    detect_protocol,
};
pub use routing::{
    DEFAULT_COOLDOWN_SECS, FailureKind, LlmPoolConfig, Pick, PickError, PoolAttemptError,
    PoolMemberConfig, ProviderPool, failure_kind,
};
pub use schema::{
    FinishReason, GenerationOptions, LlmError, LlmEvent, LlmRequest, MessageRole, ModelLimits,
    ToolChoice, ToolDefinition, Usage,
};
pub use section_index::{
    SectionIndexChunk, SectionIndexGenerator, SectionIndexOutput, SectionIndexSection,
    build_profile_metadata,
};
pub use summary::SummaryGenerator;
pub use synthesizer::{SynthesisOutput, parse_synthesis_output};
pub use token_counter::{count_chat_messages, count_system_and_query, count_tokens};
pub use usage_observer::{ChatUsageRecord, EmbeddingUsageRecord, TenantContext, UsageObserver};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiStyle {
    OpenAi,
    OpenAiResponses,
    DashScopeMultimodalEmbedding,
    DashScopeVlRerank,
    /// SiliconFlow Qwen3-VL-Embedding-8B: OpenAI-shaped `/embeddings` with
    /// multimodal `input` object array + `dimensions`. See `embedding.rs`.
    OpenAiVlEmbedding,
    /// SiliconFlow Qwen3-VL-Reranker-8B: OpenAI-shaped `/rerank` with multimodal
    /// `documents` object array. See `reranker.rs`.
    OpenAiVlRerank,
    Auto,
}

impl ApiStyle {
    pub fn from_config_str(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "openai" => Some(Self::OpenAi),
            "responses" | "openai_responses" => Some(Self::OpenAiResponses),
            "dashscope_multimodal_embedding" => Some(Self::DashScopeMultimodalEmbedding),
            "dashscope_vl_rerank" => Some(Self::DashScopeVlRerank),
            "openai_vl_embedding" => Some(Self::OpenAiVlEmbedding),
            "openai_vl_rerank" => Some(Self::OpenAiVlRerank),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }
}

impl std::fmt::Display for ApiStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::OpenAi => "openai",
            Self::OpenAiResponses => "responses",
            Self::DashScopeMultimodalEmbedding => "dashscope_multimodal_embedding",
            Self::DashScopeVlRerank => "dashscope_vl_rerank",
            Self::OpenAiVlEmbedding => "openai_vl_embedding",
            Self::OpenAiVlRerank => "openai_vl_rerank",
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
    pub enable_cache: Option<bool>,
    pub rpm_limit: Option<u32>,
    pub tpm_limit: Option<u32>,
}

impl ModelProviderConfig {
    pub fn is_configured(&self) -> bool {
        !self.api_key.is_empty() && !self.base_url.is_empty()
    }

    pub fn provider_name(&self) -> String {
        let url = self.base_url.to_ascii_lowercase();
        if url.contains("dashscope") {
            "dashscope".to_string()
        } else if url.contains("deepseek") {
            "deepseek".to_string()
        } else if url.contains("openai") {
            "openai".to_string()
        } else if url.contains("siliconflow") {
            "siliconflow".to_string()
        } else {
            "unknown".to_string()
        }
    }

    pub fn effective_rpm_limit(&self) -> u32 {
        self.rpm_limit
            .unwrap_or_else(|| provider_defaults(&self.base_url).0)
    }

    pub fn effective_tpm_limit(&self) -> u32 {
        self.tpm_limit
            .unwrap_or_else(|| provider_defaults(&self.base_url).1)
    }
}
