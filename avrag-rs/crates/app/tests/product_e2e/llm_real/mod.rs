//! Real-LLM E2E regression suite.
//!
//! These tests use production LLM providers instead of the mock LLM server.
//! They are marked `#[ignore]` by default because they:
//! - incur API cost (DeepSeek + DashScope),
//! - are non-deterministic,
//! - are slower than mock tests,
//! - may hit provider rate limits under parallel execution.
//!
//! Run serially with:
//!   cargo test -p app --test product_e2e llm_real -- --ignored --test-threads=1 --nocapture
//!
//! Required environment (loaded from the repository `.env` if not already set):
//!   AGENT_LLM_BASE_URL, AGENT_LLM_API_KEY, AGENT_LLM_MODEL
//!   MEMORY_LLM_BASE_URL, MEMORY_LLM_API_KEY, MEMORY_LLM_MODEL
//!   INGESTION_LLM_BASE_URL, INGESTION_LLM_API_KEY, INGESTION_LLM_MODEL
//!   EMBEDDING_BASE_URL, EMBEDDING_API_KEY, EMBEDDING_MODEL
//!   SEARCH_PROVIDER, SEARCH_BASE_URL, SEARCH_API_KEY (search tests only)

use crate::product_e2e::TestContext;

/// Load key/value pairs from the repository `.env` file into the process
/// environment.  This lets real-LLM tests discover credentials without
/// requiring the caller to `source .env` first.
///
/// Only sets variables that are **not** already present in the environment,
/// so explicit exports take priority.
fn load_env_from_repo_dotenv() {
    // The worktree usually does not have its own `.env`.
    // Try the worktree location first, then fall back to the main repo copy.
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    // crates/app -> crates -> avrag-rs -> e2e-analyzer -> worktrees -> .claude -> context-osv6 -> avrag-rs/.env
    let main_repo_dotenv = manifest
        .join("../../../../../../avrag-rs/.env")
        .canonicalize()
        .ok();
    let worktree_dotenv = manifest.join("../../.env").canonicalize().ok();
    let path = worktree_dotenv
        .or(main_repo_dotenv)
        .expect("repository .env file must exist for real-LLM tests");

    let content = std::fs::read_to_string(path).expect("read .env");
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let raw_value = raw_value.trim();
        // Strip surrounding quotes if present.
        let value = if raw_value.len() >= 2 {
            let first = raw_value.chars().next().unwrap();
            let last = raw_value.chars().last().unwrap();
            if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
                &raw_value[1..raw_value.len() - 1]
            } else {
                raw_value
            }
        } else {
            raw_value
        };
        if std::env::var(key).is_err() {
            unsafe { std::env::set_var(key, value) };
        }
    }
}

/// Guard that fails fast if a required real-LLM credential is missing.
fn require_real_llm_config() {
    let required = [
        "AGENT_LLM_BASE_URL",
        "AGENT_LLM_API_KEY",
        "AGENT_LLM_MODEL",
        "EMBEDDING_BASE_URL",
        "EMBEDDING_API_KEY",
        "EMBEDDING_MODEL",
    ];
    for key in &required {
        assert!(
            std::env::var(key).is_ok(),
            "real-LLM test missing required env var: {key}"
        );
    }
}

pub mod rag_real;
pub mod search_real;

impl TestContext {
    /// Create a TestContext that uses the **real** production LLM and embedding
    /// providers.  Mock search is still used unless SEARCH_API_KEY is present,
    /// because Brave Search is not the focus of V5 migration validation.
    pub async fn new_with_real_llm() -> Self {
        load_env_from_repo_dotenv();
        require_real_llm_config();

        // build_smoke with use_real_llm=true:
        // - does not override AGENT_LLM_* / EMBEDDING_* with mock values
        // - does not start mock LLM/Embedding servers
        // - still starts mock search server (Brave is not the V5 focus)
        Self::build_smoke(true, 300, None, true).await
    }
}
