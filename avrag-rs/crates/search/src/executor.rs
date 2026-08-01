use std::sync::Arc;
use std::time::Duration;

use contracts::auth_runtime::AuthContext;
use contracts::chat::ChatRequest;
use reqwest::Client;

use crate::{SearchConfig, SearchResponse, SearchStreamUpdate, provider};

/// Search-result cache TTL: web content is time-sensitive, so results are
/// cached for 30 minutes (not days).
const SEARCH_CACHE_TTL_SECS: u64 = 30 * 60;

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
    cache: Option<Arc<dyn avrag_rag_core_ports::CachePort>>,
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
        let client = builder.build().unwrap_or_else(|_| Client::new());
        Self {
            config,
            client,
            cache: None,
        }
    }

    /// Attach a result cache (Brave results are stable for a given query
    /// within a short window; repeated sub-queries in one ReAct loop then cost
    /// zero external calls).
    pub fn with_cache(mut self, cache: Arc<dyn avrag_rag_core_ports::CachePort>) -> Self {
        self.cache = Some(cache);
        self
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
    ///
    /// Results are cached per (query, vertical) for 30 minutes; cache hits
    /// return the stored response with `llm_usage` cleared (no fresh usage).
    pub async fn execute_search(
        &self,
        query: &str,
        vertical: Option<&str>,
    ) -> anyhow::Result<SearchResponse> {
        let cache_key = search_cache_key(query, vertical);
        if let Some(cache) = &self.cache {
            if let Some(raw) = cache.get(&cache_key).await {
                if let Ok(mut cached) = serde_json::from_str::<SearchResponse>(&raw) {
                    cached.llm_usage = None;
                    return Ok(cached);
                }
            }
        }
        let response = match self.normalized_provider().as_str() {
            "brave_llm_context" => {
                if vertical == Some("news") {
                    provider::execute_brave_news(&self.config, &self.client, query).await
                } else {
                    provider::execute_brave_llm_context(&self.config, &self.client, query).await
                }
            }
            provider => unsupported_provider(provider),
        }?;
        if let Some(cache) = &self.cache {
            if let Ok(raw) = serde_json::to_string(&response) {
                let _ = cache.set(&cache_key, &raw, SEARCH_CACHE_TTL_SECS).await;
            }
        }
        Ok(response)
    }
}

fn search_cache_key(query: &str, vertical: Option<&str>) -> String {
    match vertical {
        Some(v) => format!("search:brave:v1:{v}:{query}"),
        None => format!("search:brave:v1:{query}"),
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
