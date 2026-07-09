pub mod client;
pub mod embedding;
pub mod planner;
pub mod protocols;
pub mod provider_profiles;
pub mod providers;
pub mod rate_limiter;
pub mod reranker;
pub mod route;
pub mod schema;
pub mod section_index;
pub mod summary;
pub mod synthesizer;
pub mod token_counter;
pub mod usage_observer;

pub use client::{ChatMessage, LlmClient, LlmResponse, LlmUsage};
pub use embedding::{EmbeddingClient, MultiModalEmbeddingInput};
pub use planner::RetrievalPlanner;
pub use protocols::{
    AnthropicMessagesProtocol, GeminiProtocol, OpenAiChatProtocol, Protocol,
};
pub use provider_profiles::{
    AuthStyle, ProtocolKind, ProviderProfile, PROVIDER_PROFILES, api_key_url_for_provider,
    find_provider_profile,
};
pub use providers::Provider;
pub use rate_limiter::{
    RateLimitError, RateLimiter, SharedRateLimiter, default_rpm_limit, default_tpm_limit,
    provider_defaults,
};
pub use reranker::{
    MultiModalRerankDocument, MultiModalRerankResult, RerankResult, RerankerClient,
};
pub use route::{
    AnyRoute, Auth, DetectedProtocol, Endpoint, Framing, Route, RoutePatch,
    build_openai_chat_route, build_route_from_config, detect_protocol,
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
pub use usage_observer::{
    ChatUsageRecord, EmbeddingUsageRecord, TenantContext, UsageObserver,
};

#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
    ) -> anyhow::Result<LlmResponse>;

    async fn complete_with_tools(
        &self,
        messages: &[ChatMessage],
        tools: &[contracts::ToolSpec],
        temperature: Option<f32>,
    ) -> anyhow::Result<LlmResponse> {
        let _ = messages;
        let _ = tools;
        let _ = temperature;
        anyhow::bail!("Native tool calls not supported")
    }
}

#[async_trait::async_trait]
impl LlmProvider for LlmClient {
    async fn complete(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
    ) -> anyhow::Result<LlmResponse> {
        self.complete(messages, temperature).await
    }

    async fn complete_with_tools(
        &self,
        messages: &[ChatMessage],
        tools: &[contracts::ToolSpec],
        temperature: Option<f32>,
    ) -> anyhow::Result<LlmResponse> {
        self.complete_with_tools(messages, tools, temperature).await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiStyle {
    OpenAi,
    DashScopeMultimodalEmbedding,
    DashScopeVlRerank,
    Auto,
}

impl ApiStyle {
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
