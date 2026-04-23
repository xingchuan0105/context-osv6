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
        self.ensure_supported_provider()?;
        provider::execute_perplexity_agent(&self.config, &self.client, &request.query).await
    }

    pub async fn execute_stream(
        &self,
        request: &ChatRequest,
        mut on_update: impl FnMut(SearchStreamUpdate),
    ) -> anyhow::Result<SearchResponse> {
        self.ensure_supported_provider()?;
        provider::stream_perplexity_agent(
            &self.config,
            &self.client,
            &request.query,
            &mut on_update,
        )
        .await
    }

    fn ensure_supported_provider(&self) -> anyhow::Result<()> {
        if !self.config.provider.trim().eq_ignore_ascii_case("perplexity") {
            anyhow::bail!(
                "unsupported search provider: {}; only perplexity agent is supported",
                self.config.provider.trim()
            );
        }
        if self
            .config
            .perplexity_api_key
            .as_ref()
            .map_or(true, |key| key.trim().is_empty())
        {
            anyhow::bail!("Perplexity API key not configured");
        }
        Ok(())
    }
}
