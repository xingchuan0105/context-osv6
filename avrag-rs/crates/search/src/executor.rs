use avrag_auth::AuthContext;
use common::ChatRequest;
use reqwest::Client;

use crate::{SearchConfig, SearchResponse, SearchStreamUpdate, provider};

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
        match self.normalized_provider().as_str() {
            "brave_llm_context" => {
                provider::execute_brave_llm_context(&self.config, &self.client, &request.query)
                    .await
            }
            "perplexity" => {
                provider::execute_perplexity_agent(&self.config, &self.client, &request.query).await
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
            "perplexity" => {
                provider::stream_perplexity_agent(
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
}

fn unsupported_provider<T>(provider: &str) -> anyhow::Result<T> {
    anyhow::bail!(
        "unsupported search provider: {}; supported providers: brave_llm_context, perplexity",
        provider
    )
}
