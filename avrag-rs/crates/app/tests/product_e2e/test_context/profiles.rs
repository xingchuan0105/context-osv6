//! Profile-specific TestContext constructors.
//!
//! ## Smoke profile matrix (`build_smoke`)
//!
//! | Constructor | RAG (Milvus) | Worker | Mock LLM/Embed | Redis | Typical use |
//! |-------------|--------------|--------|----------------|-------|-------------|
//! | `new_smoke` | no | yes | yes | no | Auth, chat without retrieval |
//! | `new_smoke_with_rag` | yes | yes | yes | no | RAG smoke + integration |
//! | `new_smoke_with_rag_and_timeout` | yes | yes (short timeout) | yes | no | Worker timeout failure tests |
//! | `new_smoke_with_org` / `new_smoke_with_rag_and_org` | optional | yes | yes | no | Multi-tenant isolation |
//! | `new_embedding_cache` | yes | yes | yes (call counter) | yes | Embedding cache integration |
//! | `new_with_real_llm` | yes | yes | **real** LLM (+ optional real search) | optional | `llm_real` corpus tests |
//!
//! For RAG tests that need an ingested document, prefer [`super::super::fixtures::ready_rag_context`].

use super::super::llm_real::{
    ensure_search_defaults, has_real_search_credentials, load_env_from_repo_dotenv,
    require_real_llm_config,
};
use super::TestContext;
use super::builder::PersistentSmokeInfra;

/// Parameters for a streaming chat request.
pub struct ChatStreamParams<'a> {
    pub query: &'a str,
    pub agent_type: &'a str,
    pub notebook_id: &'a str,
    pub doc_scope: &'a [String],
    pub session_id: Option<&'a str>,
    pub format_hint: Option<&'a str>,
    /// When true, enables `DebugTrace` events (e.g. `prompt_snapshot`) in the SSE stream.
    pub debug: bool,
    /// When true, inject mock codegen chunk IDs for deterministic mock-LLM runs.
    pub pin_mock_chunk_ids: bool,
}

impl TestContext {
    /// Create a Smoke E2E context (no RAG).
    pub async fn new_smoke() -> Self {
        Self::build_smoke(false, 300, None, false, None, false, None).await
    }

    /// Create a Smoke E2E context with RAG enabled (Milvus + mock embedding/LLM).
    pub async fn new_smoke_with_rag() -> Self {
        Self::build_smoke(true, 300, None, false, None, false, None).await
    }

    /// Create a Smoke E2E context with RAG and a custom worker per-task timeout.
    pub async fn new_smoke_with_rag_and_timeout(worker_timeout_secs: u64) -> Self {
        Self::build_smoke(true, worker_timeout_secs, None, false, None, false, None).await
    }

    /// Create a Smoke E2E context with a specific org/user identity (no RAG).
    pub async fn new_smoke_with_org(org_id: &str, user_id: &str) -> Self {
        let identity = Some((org_id.to_string(), user_id.to_string()));
        Self::build_smoke(false, 300, identity, false, None, false, None).await
    }

    /// Create a Smoke E2E context with RAG and a specific org/user identity.
    pub async fn new_smoke_with_rag_and_org(org_id: &str, user_id: &str) -> Self {
        let identity = Some((org_id.to_string(), user_id.to_string()));
        Self::build_smoke(true, 300, identity, false, None, false, None).await
    }

    /// Embedding-cache profile: real Redis + mock embedding call counter.
    pub async fn new_embedding_cache() -> Self {
        Self::build_smoke(true, 300, None, false, None, true, None).await
    }

    /// Real-LLM profile: production LLM + embedding; real Brave when reachable.
    pub async fn new_with_real_llm() -> Self {
        load_env_from_repo_dotenv();
        require_real_llm_config();
        if has_real_search_credentials() {
            ensure_search_defaults();
        }
        Self::build_smoke(true, 300, None, true, None, false, None).await
    }

    /// Real-LLM profile with a longer ingestion timeout for full PDF books.
    pub async fn new_with_real_llm_pdf() -> Self {
        Self::new_with_real_llm_pdf_with_identity(None).await
    }

    /// Real-LLM PDF profile with a stable org/user (for corpus reuse across runs).
    pub async fn new_with_real_llm_pdf_with_identity(identity: Option<(String, String)>) -> Self {
        load_env_from_repo_dotenv();
        require_real_llm_config();
        if has_real_search_credentials() {
            ensure_search_defaults();
        }
        Self::build_smoke(true, 1200, identity, true, None, false, None).await
    }

    /// Real-LLM PDF profile with persistent PG/object-store for smoke-v5 corpus reuse.
    pub async fn new_with_real_llm_pdf_persistent_corpus(
        identity: Option<(String, String)>,
        infra: &PersistentSmokeInfra,
    ) -> Self {
        load_env_from_repo_dotenv();
        require_real_llm_config();
        if has_real_search_credentials() {
            ensure_search_defaults();
        }
        Self::build_smoke(true, 1200, identity, true, None, false, Some(infra)).await
    }
}
