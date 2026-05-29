//! Product E2E shared infrastructure.
//!
//! Design principles:
//! - HTTP black-box entry only — no direct Strategy/Runtime calls.
//! - Smoke uses real PG + Milvus + local Object Store, mocks LLM/Search/Embedding.
//! - Protocol assertions first, then deserialize to business types.

pub mod assertions;
pub mod setup;

pub mod smoke;
pub mod integration;
pub mod failure;
pub mod tenants;

use std::time::Duration;

// ---------------------------------------------------------------------------
// HTTP raw response (protocol layer)
// ---------------------------------------------------------------------------

/// Raw HTTP response from the test client.
///
/// All `ctx.chat()` / `ctx.upload_document()` helpers return this first.
/// Protocol-layer assertions operate on this type.
/// Product-layer assertions require deserializing `body_json` first.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub body_json: serde_json::Value,
}

impl HttpResponse {
    /// Deserialize the JSON body into a typed business response.
    pub fn into_business<T: serde::de::DeserializeOwned>(self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.body_json)
    }
}

// ---------------------------------------------------------------------------
// Business response types (re-exported from production code)
// ---------------------------------------------------------------------------

pub use common::{ChatResponse, Citation, DegradeTraceItem, DocumentStatus};

// ---------------------------------------------------------------------------
// Upload response (document upload)
// ---------------------------------------------------------------------------

/// Response from `POST /api/v1/notebooks/{id}/documents`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct UploadResponse {
    pub document_id: String,
    pub upload_url: String,
    #[serde(default)]
    pub status: u16,
}

// ---------------------------------------------------------------------------
// TestContext skeleton
// ---------------------------------------------------------------------------

/// Per-test execution context.
///
/// Created via `TestContext::new_smoke().await` or `new_integration().await`.
/// Automatically cleans up on drop (containers, temp dirs, worker process).
pub struct TestContext {
    pub http_client: reqwest::Client,
    pub base_url: String,
    // TODO(Phase 1): add AppState, worker handle, mock registry
    // TODO(Phase 2): add TestcontainerHandle for PG + Milvus
}

impl TestContext {
    /// Create a Smoke E2E context.
    ///
    /// - Real PG (Testcontainers)
    /// - Real Milvus (Testcontainers)
    /// - Real local Object Store (TempDir)
    /// - Mock LLM / Search / Embedding
    pub async fn new_smoke() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("reqwest client build");

        // TODO(Phase 0): start HTTP server on ephemeral port
        // TODO(Phase 1): wire AppState with mock providers
        let base_url = "http://127.0.0.1:0".to_string();

        Self {
            http_client: client,
            base_url,
        }
    }

    /// Create an Integration context.
    ///
    /// Same infrastructure as Smoke, but may use semi-real LLM.
    pub async fn new_integration() -> Self {
        // TODO(Phase 2): differentiate from smoke if needed
        Self::new_smoke().await
    }

    // -----------------------------------------------------------------------
    // HTTP helpers
    // -----------------------------------------------------------------------

    /// Upload a fixture file and return the document ID.
    pub async fn upload_document(&self, _fixture: &str) -> anyhow::Result<UploadResponse> {
        // TODO(Phase 1): implement
        todo!("upload_document")
    }

    /// Poll ingestion status until completed or timeout.
    pub async fn wait_for_ingestion(
        &self,
        _doc_id: &str,
        _timeout: Duration,
    ) -> anyhow::Result<DocumentStatus> {
        // TODO(Phase 1): implement
        todo!("wait_for_ingestion")
    }

    /// Send a chat query and return the raw HTTP response.
    pub async fn chat(
        &self,
        _query: &str,
        _doc_scope: &[String],
    ) -> anyhow::Result<HttpResponse> {
        // TODO(Phase 1): implement
        todo!("chat")
    }

    // -----------------------------------------------------------------------
    // Failure artifact capture
    // -----------------------------------------------------------------------

    /// Save debugging artifacts on test failure.
    pub fn save_failure_artifacts(&self, _test_name: &str) {
        // TODO(Phase 4): implement
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        // TODO(Phase 0): stop HTTP server, drop containers, clean temp dirs
    }
}
