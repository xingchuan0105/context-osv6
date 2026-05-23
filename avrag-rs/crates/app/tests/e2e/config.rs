//! Staging environment configuration for E2E tests.

/// Configuration loaded from environment variables.
///
/// Required: `E2E_LLM_BASE_URL`, `E2E_LLM_API_KEY`, `E2E_LLM_MODEL`.
/// Optional: `E2E_BRAVE_API_KEY`, `E2E_VECTOR_DB_URL`.
pub struct E2EConfig {
    pub llm_base_url: String,
    pub llm_api_key: String,
    pub llm_model: String,
    pub brave_api_key: Option<String>,
    pub vector_db_url: Option<String>,
}

impl E2EConfig {
    /// Load config from environment variables.
    /// Returns `None` if required variables are missing (test should skip).
    pub fn from_env() -> Option<Self> {
        let llm_base_url = std::env::var("E2E_LLM_BASE_URL").ok()?;
        let llm_api_key = std::env::var("E2E_LLM_API_KEY").ok()?;
        let llm_model = std::env::var("E2E_LLM_MODEL").ok()?;

        Some(Self {
            llm_base_url,
            llm_api_key,
            llm_model,
            brave_api_key: std::env::var("E2E_BRAVE_API_KEY").ok(),
            vector_db_url: std::env::var("E2E_VECTOR_DB_URL").ok(),
        })
    }

    /// Build an LlmClient from this config.
    pub fn llm_client(&self) -> avrag_llm::LlmClient {
        avrag_llm::LlmClient::new(avrag_llm::ModelProviderConfig {
            base_url: self.llm_base_url.clone(),
            api_key: self.llm_api_key.clone(),
            model: self.llm_model.clone(),
            timeout_ms: 30_000,
            api_style: Some(avrag_llm::ApiStyle::OpenAi),
            dimensions: None,
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        })
    }
}
