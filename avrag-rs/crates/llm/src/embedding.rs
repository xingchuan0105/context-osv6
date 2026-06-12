use crate::ModelProviderConfig;
use anyhow::Context;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::Arc;

const TEXT_EMBEDDING_BATCH_SIZE: usize = 10;
const EMBEDDING_CACHE_TTL_SECS: u64 = 7 * 24 * 60 * 60; // 7 days

fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

fn embedding_cache_key(model: &str, dimensions: Option<usize>, text_hash: &str) -> String {
    match dimensions {
        Some(d) => format!("embedding:{model}:{d}:{text_hash}"),
        None => format!("embedding:{model}:{text_hash}"),
    }
}

fn mm_embedding_cache_key(
    model: &str,
    dimension: Option<usize>,
    input: &MultiModalEmbeddingInput,
) -> String {
    let mut hasher = Sha256::new();
    if let Some(text) = input.text.as_deref() {
        hasher.update(b"text:");
        hasher.update(text.as_bytes());
    }
    if let Some(image) = input.image.as_deref() {
        hasher.update(b"image:");
        hasher.update(image.as_bytes());
    }
    for image in &input.images {
        hasher.update(b"images:");
        hasher.update(image.as_bytes());
    }
    if let Some(video) = input.video.as_deref() {
        hasher.update(b"video:");
        hasher.update(video.as_bytes());
    }
    let hash = hex::encode(hasher.finalize());
    match dimension {
        Some(d) => format!("mm_embedding:{model}:{d}:{hash}"),
        None => format!("mm_embedding:{model}:{hash}"),
    }
}

/// Default per-image token estimate for rate limiting (DashScope ~896/image observed).
pub fn default_image_token_estimate() -> usize {
    std::env::var("MM_EMBEDDING_IMAGE_TOKEN_ESTIMATE")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(896)
}

#[derive(Debug, Clone, Default)]
pub struct MultiModalEmbeddingInput {
    pub text: Option<String>,
    pub image: Option<String>,
    pub images: Vec<String>,
    pub video: Option<String>,
}

impl MultiModalEmbeddingInput {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: Some(text.into()),
            image: None,
            images: Vec::new(),
            video: None,
        }
    }

    pub fn text_image(text: impl Into<String>, image: impl Into<String>) -> Self {
        Self {
            text: Some(text.into()),
            image: Some(image.into()),
            images: Vec::new(),
            video: None,
        }
    }

    pub fn text_images(text: impl Into<String>, images: Vec<String>) -> Self {
        Self {
            text: Some(text.into()),
            image: None,
            images,
            video: None,
        }
    }

    pub fn image_count(&self) -> usize {
        usize::from(self.image.is_some()) + self.images.len()
    }

    pub fn estimate_tokens(&self) -> usize {
        let text_tokens = self
            .text
            .as_deref()
            .map(crate::count_tokens)
            .unwrap_or(0);
        let per_image = default_image_token_estimate();
        let image_tokens = self.image_count() * per_image;
        let video_tokens = usize::from(self.video.is_some()) * per_image;
        text_tokens + image_tokens + video_tokens
    }

}

fn build_dashscope_multimodal_contents(input: &MultiModalEmbeddingInput) -> Vec<serde_json::Value> {
    let mut contents = Vec::new();
    if let Some(text) = input
        .text
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        contents.push(json!({ "text": text }));
    }

    let image_refs: Vec<&str> = if !input.images.is_empty() {
        input
            .images
            .iter()
            .map(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .collect()
    } else if let Some(image) = input
        .image
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        vec![image]
    } else {
        Vec::new()
    };
    for image in image_refs {
        contents.push(json!({ "image": image }));
    }

    if let Some(video) = input
        .video
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        contents.push(json!({ "video": video }));
    }

    contents
}

#[derive(Debug, Clone)]
pub struct EmbeddingClient {
    config: ModelProviderConfig,
    client: reqwest::Client,
    rate_limiter: Option<crate::SharedRateLimiter>,
    cache: Option<Arc<avrag_cache_redis::CacheStore>>,
}

impl EmbeddingClient {
    pub fn new(config: ModelProviderConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(config.timeout_ms))
            .build()
            .expect("reqwest client should build");
        let rate_limiter = if config.is_configured() {
            let rpm = config.effective_rpm_limit();
            let tpm = config.effective_tpm_limit();
            Some(std::sync::Arc::new(crate::RateLimiter::new(rpm, tpm)))
        } else {
            None
        };
        Self {
            config,
            client,
            rate_limiter,
            cache: None,
        }
    }

    pub fn with_cache(mut self, cache: Arc<avrag_cache_redis::CacheStore>) -> Self {
        self.cache = Some(cache);
        self
    }

    fn estimate_tokens_for_texts(&self, texts: &[&str]) -> usize {
        texts.iter().map(|t| crate::count_tokens(t)).sum()
    }

    fn check_rate_limit(&self, estimated_tokens: usize) -> anyhow::Result<()> {
        if let Some(limiter) = &self.rate_limiter {
            match limiter.check_request(estimated_tokens) {
                Ok(_) => Ok(()),
                Err(crate::RateLimitError::RpmExceeded) => {
                    anyhow::bail!("Embedding rate limit exceeded: too many requests per minute")
                }
                Err(crate::RateLimitError::TpmExceeded) => {
                    anyhow::bail!("Embedding rate limit exceeded: too many tokens per minute")
                }
            }
        } else {
            Ok(())
        }
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

        let mut vectors = Vec::with_capacity(texts.len());
        let mut missing_indices = Vec::new();
        let mut missing_texts = Vec::new();

        if let Some(cache) = &self.cache {
            for (index, text) in texts.iter().enumerate() {
                let key = embedding_cache_key(
                    &self.config.model,
                    self.config.dimensions,
                    &sha256_hex(text),
                );
                match cache.get_json::<Vec<f32>>(&key).await {
                    Ok(Some(cached)) => vectors.push(cached),
                    _ => {
                        missing_indices.push(index);
                        missing_texts.push(*text);
                    }
                }
            }
        } else {
            missing_indices = (0..texts.len()).collect();
            missing_texts = texts.iter().copied().collect();
        }

        if !missing_texts.is_empty() {
            for batch in missing_texts.chunks(TEXT_EMBEDDING_BATCH_SIZE) {
                self.check_rate_limit(self.estimate_tokens_for_texts(batch))?;
                let batch_vectors = self.embed_openai_compatible_text(batch).await?;
                if let Some(cache) = &self.cache {
                    for (text, vector) in batch.iter().zip(batch_vectors.iter()) {
                        let key = embedding_cache_key(
                            &self.config.model,
                            self.config.dimensions,
                            &sha256_hex(text),
                        );
                        let _ = cache.set_json(&key, vector, EMBEDDING_CACHE_TTL_SECS).await;
                    }
                }
                for (batch_index, vector) in batch_vectors.into_iter().enumerate() {
                    let original_index = missing_indices[batch_index];
                    vectors.insert(original_index, vector);
                }
            }
        }

        Ok(vectors)
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

        let effective_dimension = dimension.or(self.config.dimensions);
        let cache_key = mm_embedding_cache_key(&self.config.model, effective_dimension, input);
        if let Some(cache) = &self.cache {
            match cache.get_json::<Vec<f32>>(&cache_key).await {
                Ok(Some(cached)) => return Ok(cached),
                _ => {}
            }
        }

        let estimated_tokens = input.estimate_tokens();
        self.check_rate_limit(estimated_tokens)?;

        let contents = build_dashscope_multimodal_contents(input);
        if contents.is_empty() {
            anyhow::bail!("multimodal embedding input is empty");
        }

        let mut parameters = serde_json::Map::new();
        parameters.insert("output_type".to_string(), json!("dense"));
        if contents.len() > 1 {
            parameters.insert("enable_fusion".to_string(), json!(true));
        }
        if let Some(dimension) = dimension.or(self.config.dimensions) {
            parameters.insert("dimension".to_string(), json!(dimension));
        }

        let request_body = json!({
            "model": self.config.model,
            "input": {
                "contents": contents
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
        struct DashScopeEmbeddingOutput {
            embeddings: Vec<DashScopeEmbeddingItem>,
        }

        #[derive(Deserialize, Default)]
        struct DashScopeUsage {
            #[serde(default)]
            image_tokens: Option<u32>,
            #[serde(default)]
            total_tokens: Option<u32>,
        }

        #[derive(Deserialize)]
        struct DashScopeEmbeddingItem {
            embedding: Vec<f32>,
            #[serde(default)]
            image_tokens: Option<u32>,
        }

        #[derive(Deserialize)]
        struct DashScopeEmbeddingResponse {
            output: DashScopeEmbeddingOutput,
            #[serde(default)]
            usage: Option<DashScopeUsage>,
        }

        let resp: DashScopeEmbeddingResponse = response
            .json()
            .await
            .context("Failed to parse DashScope multimodal embedding response")?;

        if let Some(limiter) = &self.rate_limiter {
            let actual_tokens = resp
                .usage
                .as_ref()
                .and_then(|usage| usage.image_tokens.or(usage.total_tokens))
                .or_else(|| {
                    resp.output
                        .embeddings
                        .first()
                        .and_then(|item| item.image_tokens)
                })
                .map(|value| value as usize)
                .unwrap_or(estimated_tokens);
            if actual_tokens > estimated_tokens {
                let _ = limiter.check_request(actual_tokens.saturating_sub(estimated_tokens));
            }
        }

        let vector = resp
            .output
            .embeddings
            .into_iter()
            .next()
            .map(|item| item.embedding)
            .context("DashScope multimodal embedding response did not include any vectors")?;

        if let Some(cache) = &self.cache {
            let _ = cache
                .set_json(&cache_key, &vector, EMBEDDING_CACHE_TTL_SECS)
                .await;
        }

        Ok(vector)
    }

    fn uses_dashscope_multimodal_embedding(&self) -> bool {
        matches!(
            self.config.api_style,
            Some(crate::ApiStyle::DashScopeMultimodalEmbedding)
        ) || matches!(
            self.config.model.as_str(),
            "qwen3-vl-embedding" | "tongyi-embedding-vision-plus-2026-03-06"
                | "tongyi-embedding-vision-flash-2026-03-06"
        )
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
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
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
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        };
        assert!(configured.is_configured());
    }

    #[test]
    fn test_multimodal_input_counts_modalities() {
        let input = MultiModalEmbeddingInput::text_image("diagram", "https://example.com/a.png");
        assert_eq!(super::build_dashscope_multimodal_contents(&input).len(), 2);
    }

    #[test]
    fn test_multimodal_token_estimate_uses_image_count() {
        let single = MultiModalEmbeddingInput::text_image("cap", "data:image/jpeg;base64,abc");
        let multi = MultiModalEmbeddingInput::text_images(
            "pages 1-4",
            vec![
                "data:image/jpeg;base64,a".to_string(),
                "data:image/jpeg;base64,b".to_string(),
                "data:image/jpeg;base64,c".to_string(),
                "data:image/jpeg;base64,d".to_string(),
            ],
        );
        assert!(multi.estimate_tokens() > single.estimate_tokens());
        assert!(multi.estimate_tokens() >= 4 * default_image_token_estimate());
    }

    #[test]
    fn test_pure_image_estimate_not_100_tokens() {
        let input = MultiModalEmbeddingInput {
            text: None,
            image: Some("data:image/jpeg;base64,abc".to_string()),
            images: Vec::new(),
            video: None,
        };
        assert!(input.estimate_tokens() >= default_image_token_estimate());
    }

    #[test]
    fn test_build_dashscope_contents_uses_separate_image_entries() {
        let input = MultiModalEmbeddingInput::text_images(
            "pages 1-4",
            vec!["img-a".to_string(), "img-b".to_string()],
        );
        let contents = super::build_dashscope_multimodal_contents(&input);
        assert_eq!(contents.len(), 3);
        assert_eq!(contents[0]["text"], "pages 1-4");
        assert_eq!(contents[1]["image"], "img-a");
        assert_eq!(contents[2]["image"], "img-b");
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
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        });
        assert!(client.uses_dashscope_multimodal_embedding());
    }

    /// Same input text → Redis cache hit → mock embedding HTTP called once.
    ///
    /// Skips when Redis is unavailable (no `TEST_REDIS_URL` / local docker).
    #[tokio::test]
    async fn embed_openai_compatible_text_caches_in_redis() {
        use axum::{Json, Router, routing::post};
        use serde_json::json;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let redis_url = std::env::var("TEST_REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        let cache = match avrag_cache_redis::CacheStore::new(&redis_url) {
            Ok(cache) => Arc::new(cache),
            Err(error) => {
                eprintln!(
                    "skip embed_openai_compatible_text_caches_in_redis: redis unavailable: {error}"
                );
                return;
            }
        };

        let http_calls = Arc::new(AtomicUsize::new(0));
        let call_counter = http_calls.clone();
        let app = Router::new().route(
            "/embeddings",
            post(move |Json(req): Json<serde_json::Value>| {
                call_counter.fetch_add(1, Ordering::SeqCst);
                let texts = req["input"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                    .unwrap_or_default();
                let dim = req["dimensions"].as_u64().unwrap_or(8) as usize;
                let vector: Vec<f32> = (0..dim).map(|i| 0.1 + i as f32 * 0.01).collect();
                let data: Vec<serde_json::Value> =
                    texts.iter().map(|_| json!({"embedding": vector})).collect();
                async move { Json(json!({ "data": data })) }
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock embedding listener");
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{port}");
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let client = EmbeddingClient::new(ModelProviderConfig {
            base_url: base_url.clone(),
            api_key: "sk-test".to_string(),
            model: "mock-embedding".to_string(),
            timeout_ms: 5_000,
            api_style: None,
            dimensions: Some(8),
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        })
        .with_cache(cache);

        let text = "cache-me-once";
        let first = client.embed(&[text]).await.expect("first embed");
        let second = client.embed(&[text]).await.expect("second embed");
        assert_eq!(first, second);
        assert_eq!(
            http_calls.load(Ordering::SeqCst),
            1,
            "second identical embed should hit Redis, not call HTTP again"
        );
    }
}
