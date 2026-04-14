use std::collections::BTreeSet;

use avrag_auth::AuthContext;
use common::ChatRequest;
use reqwest::Client;

use crate::{planner, provider, synthesis, SearchConfig, SearchResponse, SearchResult};

pub struct SearchExecutor {
    config: SearchConfig,
    client: Client,
}

impl SearchExecutor {
    pub fn new(config: SearchConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    pub async fn execute(
        &self,
        request: &ChatRequest,
        _auth: &AuthContext,
    ) -> anyhow::Result<SearchResponse> {
        if !self.config.provider.trim().eq_ignore_ascii_case("mock")
            && (self.config.api_key.trim().is_empty()
                && self
                    .config
                    .perplexity_api_key
                    .as_ref()
                    .map_or(true, |k| k.trim().is_empty()))
            && self.config.base_url.trim().is_empty()
        {
            anyhow::bail!("external search provider not configured");
        }

        if self
            .config
            .provider
            .trim()
            .eq_ignore_ascii_case("perplexity")
        {
            return provider::execute_perplexity_agent(&self.config, &self.client, &request.query).await;
        }

        let (plan, planner_usage) = self.plan_query(&request.query).await?;
        let mut combined = Vec::new();
        let mut seen_urls = BTreeSet::new();

        for query in &plan.sub_queries {
            let results = self.call_provider(query).await?;
            for result in results {
                if seen_urls.insert(result.url.clone()) {
                    combined.push(result);
                }
                if combined.len() >= self.config.max_results {
                    break;
                }
            }
            if combined.len() >= self.config.max_results {
                break;
            }
        }

        if combined.is_empty() && self.config.citation_required {
            anyhow::bail!("search returned no results");
        }

        let synthesized_answer =
            synthesis::synthesize_answer(&request.query, &combined, self.config.synthesizer.as_ref())
                .await?;

        Ok(SearchResponse {
            query_type: plan.query_type,
            sub_queries: plan.sub_queries,
            results: combined,
            synthesized_answer,
            planner_usage,
        })
    }

    async fn plan_query(
        &self,
        query: &str,
    ) -> anyhow::Result<(planner::SearchPlan, Option<avrag_llm::LlmUsage>)> {
        planner::plan_query_with_usage(query, &self.config).await
    }

    async fn call_provider(&self, query: &str) -> anyhow::Result<Vec<SearchResult>> {
        provider::search_by_provider(&self.config, &self.client, query).await
    }
}
