use std::sync::Arc;
use std::time::Duration;

use contracts::auth_runtime::AuthContext;
use contracts::chat::ChatRequest;
use reqwest::Client;
use tracing::{info, warn};

use crate::{SearchConfig, SearchResponse, SearchStreamUpdate, provider};

/// Search-result cache TTL: web content is time-sensitive, so results are
/// cached for 30 minutes (not days).
const SEARCH_CACHE_TTL_SECS: u64 = 30 * 60;

/// Object-safe abstraction over `SearchExecutor::execute_search`.
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

    pub fn with_cache(mut self, cache: Arc<dyn avrag_rag_core_ports::CachePort>) -> Self {
        self.cache = Some(cache);
        self
    }

    pub async fn execute(
        &self,
        request: &ChatRequest,
        _auth: &AuthContext,
    ) -> anyhow::Result<SearchResponse> {
        self.execute_search(&request.query, None).await
    }

    pub async fn execute_stream(
        &self,
        request: &ChatRequest,
        mut on_update: impl FnMut(SearchStreamUpdate),
    ) -> anyhow::Result<SearchResponse> {
        match self.normalized_provider().as_str() {
            "qwen_web" | "qwen" => {
                provider::stream_qwen_web(&self.config, &self.client, &request.query, &mut on_update)
                    .await
            }
            "brave_llm_context" => {
                provider::stream_brave_llm_context(
                    &self.config,
                    &self.client,
                    &request.query,
                    &mut on_update,
                )
                .await
            }
            "deepseek_web" => {
                provider::stream_deepseek_web(
                    &self.config,
                    &self.client,
                    &request.query,
                    &mut on_update,
                )
                .await
            }
            "deepseek_web_brave" | "deepseek" => {
                match provider::stream_deepseek_web(
                    &self.config,
                    &self.client,
                    &request.query,
                    &mut on_update,
                )
                .await
                {
                    Ok(r) if !r.results.is_empty() => Ok(r),
                    Ok(empty) => {
                        warn!(
                            target: "search",
                            "deepseek_web returned no results; falling back to Brave"
                        );
                        if self.config.api_key.trim().is_empty() {
                            return Ok(empty);
                        }
                        provider::stream_brave_llm_context(
                            &self.config,
                            &self.client,
                            &request.query,
                            &mut on_update,
                        )
                        .await
                    }
                    Err(e) => {
                        warn!(
                            target: "search",
                            error = %e,
                            "deepseek_web failed; falling back to Brave"
                        );
                        if self.config.api_key.trim().is_empty() {
                            return Err(e);
                        }
                        provider::stream_brave_llm_context(
                            &self.config,
                            &self.client,
                            &request.query,
                            &mut on_update,
                        )
                        .await
                    }
                }
            }
            provider => unsupported_provider(provider),
        }
    }

    fn normalized_provider(&self) -> String {
        self.config.provider.trim().to_ascii_lowercase()
    }

    pub fn provider(&self) -> &str {
        match self.normalized_provider().as_str() {
            "qwen_web" | "qwen" => "qwen_web",
            "brave_llm_context" => "brave_llm_context",
            "deepseek_web" => "deepseek_web",
            "deepseek_web_brave" | "deepseek" => "deepseek_web_brave",
            _ => "unknown",
        }
    }

    /// Execute a single search query (SaC `client.web`, Web Worker host leaf).
    /// Runs CRW auto-scrape when configured (`WEB_AUTO_SCRAPE*`; thin-snippet gate).
    pub async fn execute_search(
        &self,
        query: &str,
        vertical: Option<&str>,
    ) -> anyhow::Result<SearchResponse> {
        let cache_key = search_cache_key(self.provider(), query, vertical);
        if let Some(cache) = &self.cache {
            if let Some(raw) = cache.get(&cache_key).await {
                if let Ok(mut cached) = serde_json::from_str::<SearchResponse>(&raw) {
                    cached.llm_usage = None;
                    return Ok(cached);
                }
            }
        }

        let response = self.dispatch_search(query, vertical).await?;

        if let Some(cache) = &self.cache {
            if let Ok(raw) = serde_json::to_string(&response) {
                let _ = cache.set(&cache_key, &raw, SEARCH_CACHE_TTL_SECS).await;
            }
        }
        Ok(response)
    }

    async fn dispatch_search(
        &self,
        query: &str,
        vertical: Option<&str>,
    ) -> anyhow::Result<SearchResponse> {
        match self.normalized_provider().as_str() {
            "qwen_web" | "qwen" => {
                // Qwen native web_search has no news vertical — all queries go the same path.
                let mut resp = provider::execute_qwen_web(&self.config, &self.client, query).await?;
                trim_results(&mut resp, self.config.max_results);
                self.enrich_with_auto_scrape(&mut resp).await;
                info!(
                    target: "search",
                    provider = "qwen_web",
                    n = resp.results.len(),
                    "search ok"
                );
                Ok(resp)
            }
            "brave_llm_context" => self.execute_brave(query, vertical).await,
            "deepseek_web" => {
                // News vertical: DeepSeek has no news endpoint — use Brave if key present.
                if vertical == Some("news") {
                    return self.execute_brave(query, vertical).await;
                }
                let mut resp =
                    provider::execute_deepseek_web(&self.config, &self.client, query).await?;
                trim_results(&mut resp, self.config.max_results);
                self.enrich_with_auto_scrape(&mut resp).await;
                Ok(resp)
            }
            "deepseek_web_brave" | "deepseek" => {
                if vertical == Some("news") {
                    return self.execute_brave(query, vertical).await;
                }
                match provider::execute_deepseek_web(&self.config, &self.client, query).await {
                    Ok(mut r) if !r.results.is_empty() => {
                        trim_results(&mut r, self.config.max_results);
                        self.enrich_with_auto_scrape(&mut r).await;
                        info!(
                            target: "search",
                            provider = "deepseek_web",
                            n = r.results.len(),
                            "search ok"
                        );
                        Ok(r)
                    }
                    Ok(empty) => {
                        warn!(
                            target: "search",
                            "deepseek_web empty results; brave fallback"
                        );
                        if self.config.api_key.trim().is_empty() {
                            return Ok(empty);
                        }
                        self.execute_brave(query, vertical).await
                    }
                    Err(e) => {
                        warn!(
                            target: "search",
                            error = %e,
                            "deepseek_web error; brave fallback"
                        );
                        if self.config.api_key.trim().is_empty() {
                            return Err(e);
                        }
                        self.execute_brave(query, vertical).await
                    }
                }
            }
            provider => unsupported_provider(provider),
        }
    }

    async fn execute_brave(
        &self,
        query: &str,
        vertical: Option<&str>,
    ) -> anyhow::Result<SearchResponse> {
        let mut resp = if vertical == Some("news") {
            provider::execute_brave_news(&self.config, &self.client, query).await?
        } else {
            provider::execute_brave_llm_context(&self.config, &self.client, query).await?
        };
        trim_results(&mut resp, self.config.max_results);
        // Brave often has thick snippets; enrich only thin ones (min_snippet gate).
        self.enrich_with_auto_scrape(&mut resp).await;
        Ok(resp)
    }

    async fn enrich_with_auto_scrape(&self, resp: &mut SearchResponse) {
        crate::crw::auto_scrape_enrich_results(&self.config, &self.client, &mut resp.results).await;
    }
}

fn trim_results(resp: &mut SearchResponse, max: usize) {
    let max = max.max(1);
    if resp.results.len() > max {
        resp.results.truncate(max);
        for (i, r) in resp.results.iter_mut().enumerate() {
            r.citation_index = Some(i + 1);
        }
    }
}

fn search_cache_key(provider: &str, query: &str, vertical: Option<&str>) -> String {
    match vertical {
        Some(v) => format!("search:{provider}:v1:{v}:{query}"),
        None => format!("search:{provider}:v1:{query}"),
    }
}

fn unsupported_provider<T>(provider: &str) -> anyhow::Result<T> {
    anyhow::bail!(
        "unsupported search provider: {provider}; supported: qwen_web, deepseek_web_brave, deepseek_web, brave_llm_context"
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
