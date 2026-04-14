use crate::ModelProviderConfig;
use anyhow::Context;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Clone, Default)]
pub struct MultiModalEmbeddingInput {
    pub text: Option<String>,
    pub image: Option<String>,
    pub video: Option<String>,
}

impl MultiModalEmbeddingInput {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: Some(text.into()),
            image: None,
            video: None,
        }
    }

    pub fn text_image(text: impl Into<String>, image: impl Into<String>) -> Self {
        Self {
            text: Some(text.into()),
            image: Some(image.into()),
            video: None,
        }
    }

    fn modality_count(&self) -> usize {
        usize::from(self.text.is_some())
            + usize::from(self.image.is_some())
            + usize::from(self.video.is_some())
    }
}

#[derive(Debug, Clone)]
pub struct EmbeddingClient {
    config: ModelProviderConfig,
    client: reqwest::Client,
}

impl EmbeddingClient {
    pub fn new(config: ModelProviderConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(config.timeout_ms))
            .build()
            .expect("reqwest client should build");
        Self { config, client }
    }

    pub async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        if self.uses_dashscope_multimodal_embedding() {
            let mut vectors = Vec::with_capacity(texts.len());
            for text in texts {
                vectors.push(
                    self.embed_multimodal_fused(&MultiModalEmbeddingInput::text(*text), None)
                        .await?,
                );
            }
            return Ok(vectors);
        }

        self.embed_openai_compatible_text(texts).await
    }

    pub async fn embed_multimodal_fused(
        &self,
        input: &MultiModalEmbeddingInput,
        dimension: Option<usize>,
    ) -> anyhow::Result<Vec<f32>> {
        if !self.config.is_configured() {
            anyhow::bail!("Embedding API not configured: API key or base_url is empty");
        }
        if !self.uses_dashscope_multimodal_embedding() {
            anyhow::bail!(
                "embed_multimodal_fused requires a DashScope multimodal embedding config"
            );
        }

        let mut content = serde_json::Map::new();
        if let Some(text) = input
            .text
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            content.insert("text".to_string(), json!(text));
        }
        if let Some(image) = input
            .image
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            content.insert("image".to_string(), json!(image));
        }
        if let Some(video) = input
            .video
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            content.insert("video".to_string(), json!(video));
        }
        if content.is_empty() {
            anyhow::bail!("multimodal embedding input is empty");
        }

        let mut parameters = serde_json::Map::new();
        parameters.insert("output_type".to_string(), json!("dense"));
        if input.modality_count() > 1 {
            parameters.insert("enable_fusion".to_string(), json!(true));
        }
        if let Some(dimension) = dimension.or(self.config.dimensions) {
            parameters.insert("dimension".to_string(), json!(dimension));
        }

        let request_body = json!({
            "model": self.config.model,
            "input": {
                "contents": [serde_json::Value::Object(content)]
            },
            "parameters": serde_json::Value::Object(parameters),
        });

        let response = self
            .client
            .post(&self.config.base_url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .context("Failed to send DashScope multimodal embedding request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "DashScope multimodal embedding API error {}: {}",
                status,
                body
            );
        }

        #[derive(Deserialize)]
        struct DashScopeEmbeddingResponse {
            output: DashScopeEmbeddingOutput,
        }

        #[derive(Deserialize)]
        struct DashScopeEmbeddingOutput {
            embeddings: Vec<DashScopeEmbeddingItem>,
        }

        #[derive(Deserialize)]
        struct DashScopeEmbeddingItem {
            embedding: Vec<f32>,
        }

        let resp: DashScopeEmbeddingResponse = response
            .json()
            .await
            .context("Failed to parse DashScope multimodal embedding response")?;

        resp.output
            .embeddings
            .into_iter()
            .next()
            .map(|item| item.embedding)
            .context("DashScope multimodal embedding response did not include any vectors")
    }

    fn uses_dashscope_multimodal_embedding(&self) -> bool {
        matches!(
            self.config.api_style,
            Some(crate::ApiStyle::DashScopeMultimodalEmbedding)
        ) || self.config.model == "qwen3-vl-embedding"
    }

    async fn embed_openai_compatible_text(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        if !self.config.is_configured() {
            anyhow::bail!("Embedding API not configured: API key or base_url is empty");
        }

        let mut request_body = json!({
            "model": self.config.model,
            "input": texts,
        });
        if let Some(dimension) = self.config.dimensions {
            request_body["dimensions"] = json!(dimension);
        }

        let response = self
            .client
            .post(format!(
                "{}/embeddings",
                self.config.base_url.trim_end_matches('/')
            ))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .context("Failed to send embedding request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Embedding API error {}: {}", status, body);
        }

        #[derive(Deserialize)]
        struct EmbeddingResponse {
            data: Vec<EmbeddingData>,
        }

        #[derive(Deserialize)]
        struct EmbeddingData {
            embedding: Vec<f32>,
        }

        let resp: EmbeddingResponse = response
            .json()
            .await
            .context("Failed to parse embedding response")?;

        Ok(resp.data.into_iter().map(|d| d.embedding).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_provider_config_is_configured() {
        let empty = ModelProviderConfig {
            base_url: "".to_string(),
            api_key: "".to_string(),
            model: "test".to_string(),
            timeout_ms: 1000,
            api_style: None,
            dimensions: None,
            enable_thinking: None,
        };
        assert!(!empty.is_configured());

        let configured = ModelProviderConfig {
            base_url: "https://api.example.com".to_string(),
            api_key: "sk-test".to_string(),
            model: "test".to_string(),
            timeout_ms: 1000,
            api_style: None,
            dimensions: None,
            enable_thinking: None,
        };
        assert!(configured.is_configured());
    }

    #[test]
    fn test_multimodal_input_counts_modalities() {
        let input = MultiModalEmbeddingInput::text_image("diagram", "https://example.com/a.png");
        assert_eq!(input.modality_count(), 2);
    }

    #[test]
    fn test_dashscope_multimodal_detection_by_model() {
        let client = EmbeddingClient::new(ModelProviderConfig {
            base_url: "https://dashscope.aliyuncs.com/api/v1/services/embeddings/multimodal-embedding/multimodal-embedding".to_string(),
            api_key: "sk-test".to_string(),
            model: "qwen3-vl-embedding".to_string(),
            timeout_ms: 1000,
            api_style: None,
            dimensions: None,
            enable_thinking: None,
        });
        assert!(client.uses_dashscope_multimodal_embedding());
    }
}
