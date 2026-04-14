use crate::ModelProviderConfig;
use anyhow::Context;
use serde::Deserialize;
use serde_json::json;

pub struct RerankerClient {
    config: ModelProviderConfig,
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub enum MultiModalRerankDocument {
    Text(String),
    Image(String),
    Video(String),
}

#[derive(Debug, Clone)]
pub struct MultiModalRerankResult {
    pub index: usize,
    pub score: f32,
}

impl RerankerClient {
    pub fn new(config: ModelProviderConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(config.timeout_ms))
            .build()
            .expect("reqwest client should build");
        Self { config, client }
    }

    pub async fn rerank(
        &self,
        query: &str,
        documents: &[String],
    ) -> anyhow::Result<Vec<RerankResult>> {
        if !self.config.is_configured() {
            anyhow::bail!("Reranker not configured");
        }

        if self.uses_dashscope_vl_rerank() {
            let mm_documents = documents
                .iter()
                .cloned()
                .map(MultiModalRerankDocument::Text)
                .collect::<Vec<_>>();
            let ranked = self
                .rerank_multimodal_text_query(query, &mm_documents, documents.len())
                .await?;
            return Ok(ranked
                .into_iter()
                .map(|r| RerankResult {
                    index: r.index,
                    document: documents.get(r.index).cloned().unwrap_or_default(),
                    score: r.score,
                })
                .collect());
        }

        let request_body = json!({
            "model": self.config.model,
            "query": query,
            "documents": documents,
        });

        let response = self
            .client
            .post(format!(
                "{}/rerank",
                self.config.base_url.trim_end_matches('/')
            ))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .context("Failed to send rerank request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Rerank API error {}: {}", status, body);
        }

        #[derive(Deserialize)]
        struct RerankResponse {
            results: Vec<RerankResultItem>,
        }

        #[derive(Deserialize)]
        struct RerankResultItem {
            index: usize,
            relevance_score: f32,
        }

        let resp: RerankResponse = response
            .json()
            .await
            .context("Failed to parse rerank response")?;

        Ok(resp
            .results
            .into_iter()
            .map(|r| RerankResult {
                index: r.index,
                document: documents.get(r.index).cloned().unwrap_or_default(),
                score: r.relevance_score,
            })
            .collect())
    }

    pub async fn rerank_multimodal_text_query(
        &self,
        query: &str,
        documents: &[MultiModalRerankDocument],
        top_n: usize,
    ) -> anyhow::Result<Vec<MultiModalRerankResult>> {
        if !self.config.is_configured() {
            anyhow::bail!("Reranker not configured");
        }
        if !self.uses_dashscope_vl_rerank() {
            anyhow::bail!("rerank_multimodal_text_query requires a qwen3-vl-rerank config");
        }

        let request_body = json!({
            "model": self.config.model,
            "input": {
                "query": { "text": query },
                "documents": documents.iter().map(multimodal_document_to_json).collect::<Vec<_>>()
            },
            "parameters": {
                "return_documents": false,
                "top_n": top_n,
                "instruct": "Given a web search query, retrieve relevant passages that answer the query."
            }
        });

        let response = self
            .client
            .post(&self.config.base_url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .context("Failed to send multimodal rerank request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Multimodal rerank API error {}: {}", status, body);
        }

        #[derive(Deserialize)]
        struct DashScopeRerankResponse {
            output: DashScopeRerankOutput,
        }

        #[derive(Deserialize)]
        struct DashScopeRerankOutput {
            results: Vec<DashScopeRerankItem>,
        }

        #[derive(Deserialize)]
        struct DashScopeRerankItem {
            index: usize,
            relevance_score: f32,
        }

        let resp: DashScopeRerankResponse = response
            .json()
            .await
            .context("Failed to parse multimodal rerank response")?;

        Ok(resp
            .output
            .results
            .into_iter()
            .map(|result| MultiModalRerankResult {
                index: result.index,
                score: result.relevance_score,
            })
            .collect())
    }

    fn uses_dashscope_vl_rerank(&self) -> bool {
        matches!(
            self.config.api_style,
            Some(crate::ApiStyle::DashScopeVlRerank)
        ) || self.config.model == "qwen3-vl-rerank"
    }
}

fn multimodal_document_to_json(document: &MultiModalRerankDocument) -> serde_json::Value {
    match document {
        MultiModalRerankDocument::Text(text) => json!({ "text": text }),
        MultiModalRerankDocument::Image(image) => json!({ "image": image }),
        MultiModalRerankDocument::Video(video) => json!({ "video": video }),
    }
}

#[derive(Debug, Clone)]
pub struct RerankResult {
    pub index: usize,
    pub document: String,
    pub score: f32,
}
