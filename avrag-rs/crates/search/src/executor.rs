use std::time::Duration;

use avrag_auth::AuthContext;
use contracts::chat::ChatRequest;
use reqwest::Client;

use crate::{SearchConfig, SearchResponse, SearchStreamUpdate, provider};

/// Object-safe abstraction over `SearchExecutor::execute_search`.
///
/// The web-search agent's ReAct loop only dispatches single-query searches
/// (with optional vertical), so the trait surface is intentionally narrow.
/// `SearchExecutor` is the production implementor; tests can plug in fakes
/// without spinning up a real HTTP server.
#[async_trait::async_trait]
pub trait SearchProvider: Send + Sync {
    async fn execute_search(
        &self,
        query: &str,
        vertical: Option<&str>,
    ) -> anyhow::Result<SearchResponse>;
}

pub struct SearchExecutor {
    config: SearchConfig,
    client: Client,
}

impl SearchExecutor {
    pub fn new(config: SearchConfig) -> Self {
        crate::proxy::sync_resolved_proxy_env();
        let timeout = Duration::from_millis(config.timeout_ms.max(1));
        let mut builder = Client::builder().timeout(timeout);
        if let Some(proxy_url) = crate::proxy::resolved_proxy_url() {
            if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
                builder = builder.proxy(proxy);
            }
        }
        let client = builder
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { config, client }
    }

    pub async fn execute(
        &self,
        request: &ChatRequest,
        _auth: &AuthContext,
    ) -> anyhow::Result<SearchResponse> {
        match self.normalized_provider().as_str() {
            "brave_llm_context" => {
                provider::execute_brave_llm_context(&self.config, &self.client, &request.query)
                    .await
            }
            provider => unsupported_provider(provider),
        }
    }

    pub async fn execute_stream(
        &self,
        request: &ChatRequest,
        mut on_update: impl FnMut(SearchStreamUpdate),
    ) -> anyhow::Result<SearchResponse> {
        match self.normalized_provider().as_str() {
            "brave_llm_context" => {
                provider::stream_brave_llm_context(
                    &self.config,
                    &self.client,
                    &request.query,
                    &mut on_update,
                )
                .await
            }
            provider => unsupported_provider(provider),
        }
    }

    fn normalized_provider(&self) -> String {
        self.config.provider.trim().to_ascii_lowercase()
    }

    pub fn provider(&self) -> &str {
        match self.normalized_provider().as_str() {
            "brave_llm_context" => "brave_llm_context",
            _ => "unknown",
        }
    }

    /// Execute a single search query without streaming or auth requirements.
    /// Used by the web-search agent for parallel sub-query execution.
    pub async fn execute_search(
        &self,
        query: &str,
        vertical: Option<&str>,
    ) -> anyhow::Result<SearchResponse> {
        match self.normalized_provider().as_str() {
            "brave_llm_context" => {
                if vertical == Some("news") {
                    provider::execute_brave_news(&self.config, &self.client, query).await
                } else {
                    provider::execute_brave_llm_context(&self.config, &self.client, query).await
                }
            }
            provider => unsupported_provider(provider),
        }
    }
}

fn unsupported_provider<T>(provider: &str) -> anyhow::Result<T> {
    anyhow::bail!(
        "unsupported search provider: {}; supported providers: brave_llm_context",
        provider
    )
}

#[async_trait::async_trait]
impl SearchProvider for SearchExecutor {
    async fn execute_search(
        &self,
        query: &str,
        vertical: Option<&str>,
    ) -> anyhow::Result<SearchResponse> {
        SearchExecutor::execute_search(self, query, vertical).await
    }
}
