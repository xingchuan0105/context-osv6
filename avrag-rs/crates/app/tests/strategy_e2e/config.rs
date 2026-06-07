//! Staging environment configuration for E2E tests.

/// Configuration loaded from environment variables.
///
/// Required for all tests: `E2E_LLM_BASE_URL`, `E2E_LLM_API_KEY`, `E2E_LLM_MODEL`.
/// Required for Search: `E2E_BRAVE_API_KEY`.
/// Required for RAG: `E2E_EMBEDDING_BASE_URL`, `E2E_EMBEDDING_API_KEY`,
///   `E2E_MILVUS_URL`.
pub struct E2EConfig {
    pub llm_base_url: String,
    pub llm_api_key: String,
    pub llm_model: String,
    pub brave_api_key: Option<String>,
    pub embedding_base_url: Option<String>,
    pub embedding_api_key: Option<String>,
    pub embedding_model: Option<String>,
    pub milvus_url: Option<String>,
    pub milvus_token: Option<String>,
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
            embedding_base_url: std::env::var("E2E_EMBEDDING_BASE_URL").ok(),
            embedding_api_key: std::env::var("E2E_EMBEDDING_API_KEY").ok(),
            embedding_model: std::env::var("E2E_EMBEDDING_MODEL").ok(),
            milvus_url: std::env::var("E2E_MILVUS_URL").ok(),
            milvus_token: std::env::var("E2E_MILVUS_TOKEN").ok(),
        })
    }

    /// Validate that all variables required for Chat tests are present.
    /// Returns Ok(()) on success, or Err with a list of missing variables.
    pub fn validate_for_chat(&self) -> Result<(), Vec<String>> {
        let mut missing = Vec::new();
        if self.llm_base_url.is_empty() {
            missing.push("E2E_LLM_BASE_URL".to_string());
        }
        if self.llm_api_key.is_empty() {
            missing.push("E2E_LLM_API_KEY".to_string());
        }
        if self.llm_model.is_empty() {
            missing.push("E2E_LLM_MODEL".to_string());
        }
        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
        }
    }

    /// Validate that all variables required for Search tests are present.
    pub fn validate_for_search(&self) -> Result<(), Vec<String>> {
        let mut missing = Vec::new();
        if let Err(m) = self.validate_for_chat() {
            missing.extend(m);
        }
        if self
            .brave_api_key
            .as_ref()
            .map(|s| s.is_empty())
            .unwrap_or(true)
        {
            missing.push("E2E_BRAVE_API_KEY".to_string());
        }
        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
        }
    }

    /// Validate that all variables required for RAG tests are present.
    pub fn validate_for_rag(&self) -> Result<(), Vec<String>> {
        let mut missing = Vec::new();
        if let Err(m) = self.validate_for_chat() {
            missing.extend(m);
        }
        if self
            .embedding_base_url
            .as_ref()
            .map(|s| s.is_empty())
            .unwrap_or(true)
        {
            missing.push("E2E_EMBEDDING_BASE_URL".to_string());
        }
        if self
            .embedding_api_key
            .as_ref()
            .map(|s| s.is_empty())
            .unwrap_or(true)
        {
            missing.push("E2E_EMBEDDING_API_KEY".to_string());
        }
        if self
            .milvus_url
            .as_ref()
            .map(|s| s.is_empty())
            .unwrap_or(true)
        {
            missing.push("E2E_MILVUS_URL".to_string());
        }
        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
        }
    }

    /// Build an LlmClient from this config.
    pub fn llm_client(&self) -> avrag_llm::LlmClient {
        // Strip trailing slash to avoid double-slash in URLs like "{base}/chat/completions"
        let base_url = self.llm_base_url.trim_end_matches('/').to_string();
        avrag_llm::LlmClient::new(avrag_llm::ModelProviderConfig {
            base_url,
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

    /// Build an EmbeddingClient from this config (RAG tests).
    /// Panics if embedding config is incomplete — call `validate_for_rag()` first.
    pub fn embedding_client(&self) -> avrag_llm::EmbeddingClient {
        let base_url = self
            .embedding_base_url
            .as_ref()
            .expect("validate_for_rag() not called")
            .trim_end_matches('/')
            .to_string();
        avrag_llm::EmbeddingClient::new(avrag_llm::ModelProviderConfig {
            base_url,
            api_key: self.embedding_api_key.clone().unwrap_or_default(),
            model: self
                .embedding_model
                .clone()
                .unwrap_or_else(|| "text-embedding-v4".to_string()),
            timeout_ms: 30_000,
            api_style: Some(avrag_llm::ApiStyle::OpenAi),
            dimensions: Some(1024),
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        })
    }

    /// Generate a unique run_id for this test session.
    pub fn generate_run_id() -> String {
        let now = chrono::Utc::now();
        let uuid = uuid::Uuid::new_v4().to_string();
        let short = &uuid[..8];
        format!("e2e_{}_{}", now.format("%Y%m%d-%H%M%S"), short)
    }

    /// Collect environment snapshot for metadata.
    pub fn environment_snapshot() -> serde_json::Value {
        serde_json::json!({
            "git_commit": run_git_command(&["rev-parse", "HEAD"]),
            "git_branch": run_git_command(&["rev-parse", "--abbrev-ref", "HEAD"]),
            "rust_toolchain": run_shell_command("rustc --version"),
            "node_version": run_shell_command("node --version"),
            "playwright_version": run_shell_command("npx playwright --version"),
        })
    }
}

fn run_git_command(args: &[&str]) -> String {
    std::process::Command::new("git")
        .args(args)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn run_shell_command(cmd: &str) -> String {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return String::new();
    }
    std::process::Command::new(parts[0])
        .args(&parts[1..])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
        .trim()
        .to_string()
}
