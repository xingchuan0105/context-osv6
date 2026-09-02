use crate::ModelProviderConfig;
use crate::embed_rate_limit::{EmbedLane, EmbedRateGate, EmbedRateRequest};
use crate::usage_observer::{EmbeddingUsageRecord, TenantContext, UsageObserver};
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

/// L2-normalize; zero vectors pass through unchanged (avoids NaN).
pub fn l2_normalized(v: &[f32]) -> Vec<f32> {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm <= f32::EPSILON {
        return v.to_vec();
    }
    v.iter().map(|x| x / norm).collect()
}

/// Client-side multimodal fusion: L2 each part → mean → L2 (SiliconFlow VL).
/// Single-part: pass through **without** L2 (DashScope / SF single-item parity).
pub fn fuse_part_vectors(parts: &[Vec<f32>]) -> anyhow::Result<Vec<f32>> {
    let (first, rest) = parts
        .split_first()
        .ok_or_else(|| anyhow::anyhow!("fuse_part_vectors: empty parts"))?;
    if rest.is_empty() {
        return Ok(first.clone());
    }
    let dim = first.len();
    let mut acc = vec![0f32; dim];
    for v in std::iter::once(first).chain(rest.iter()) {
        anyhow::ensure!(
            v.len() == dim,
            "embedding part dim mismatch: {} vs {dim}",
            v.len()
        );
        for (a, b) in acc.iter_mut().zip(l2_normalized(v)) {
            *a += b;
        }
    }
    let count = (rest.len() + 1) as f32;
    for a in acc.iter_mut() {
        *a /= count;
    }
    Ok(l2_normalized(&acc))
}

/// Single type source: `avrag_rag_core_ports` (T5 deepen).
pub use avrag_rag_core_ports::MultiModalEmbeddingInput;

fn estimate_mm_tokens(input: &MultiModalEmbeddingInput) -> usize {
    let text_tokens = input.text.as_deref().map(crate::count_tokens).unwrap_or(0);
    let per_image = default_image_token_estimate();
    let image_tokens = input.image_count() * per_image;
    let video_tokens = usize::from(input.video.is_some()) * per_image;
    text_tokens + image_tokens + video_tokens
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

#[derive(Clone)]
pub struct EmbeddingClient {
    config: ModelProviderConfig,
    client: reqwest::Client,
    rate_limiter: Option<crate::SharedRateLimiter>,
    rate_gate: Option<Arc<dyn EmbedRateGate>>,
    cache: Option<Arc<dyn avrag_rag_core_ports::CachePort>>,
    feature: String,
    observer: Option<(Arc<dyn UsageObserver>, TenantContext)>,
}

impl std::fmt::Debug for EmbeddingClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbeddingClient")
            .field("config", &self.config)
            .field("feature", &self.feature)
            .field("has_observer", &self.observer.is_some())
            .finish_non_exhaustive()
    }
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
            rate_gate: None,
            cache: None,
            feature: "document_embedding".to_string(),
            observer: None,
        }
    }

    pub fn with_cache(mut self, cache: Arc<dyn avrag_rag_core_ports::CachePort>) -> Self {
        self.cache = Some(cache);
        self
    }

    pub fn with_rate_gate(mut self, gate: Arc<dyn EmbedRateGate>) -> Self {
        self.rate_gate = Some(gate);
        self
    }

    pub fn with_feature(mut self, feature: impl std::fmt::Display) -> Self {
        self.feature = feature.to_string();
        self
    }

    pub fn with_observer(
        mut self,
        observer: Arc<dyn UsageObserver>,
        tenant: TenantContext,
    ) -> Self {
        self.observer = Some((observer, tenant));
        self
    }

    async fn record_embedding_usage(&self, estimated_tokens: u32, actual_tokens: Option<u32>) {
        let Some((observer, tenant)) = &self.observer else {
            return;
        };
        let record = EmbeddingUsageRecord {
            estimated_tokens,
            actual_tokens,
            provider: self.config.provider_name(),
            model: self.config.model.clone(),
            feature: self.feature.clone(),
        };
        observer.record_embedding(tenant, &record).await;
    }

    fn estimate_tokens_for_texts(&self, texts: &[&str]) -> usize {
        texts.iter().map(|t| crate::count_tokens(t)).sum()
    }

    async fn acquire_embed_permit(&self, estimated_tokens: usize) -> anyhow::Result<()> {
        if let Some(gate) = &self.rate_gate {
            return gate
                .acquire(EmbedRateRequest {
                    lane: EmbedLane::from_feature(&self.feature),
                    tokens: estimated_tokens,
                })
                .await;
        }
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

        // Slot-based reassembly: batch offset must index into `missing_indices`, not
        // the local batch index alone (old `Vec::insert` path was O(n²) and wrong for
        // batch>0 — multi-chunk ingestion hung at high CPU on ~hundreds of vectors).
        let mut slots: Vec<Option<Vec<f32>>> = vec![None; texts.len()];
        let mut missing_indices = Vec::new();
        let mut missing_texts = Vec::new();

        if let Some(cache) = &self.cache {
            for (index, text) in texts.iter().enumerate() {
                let key = embedding_cache_key(
                    &self.config.model,
                    self.config.dimensions,
                    &sha256_hex(text),
                );
                match cache
                    .get(&key)
                    .await
                    .and_then(|raw| serde_json::from_str(&raw).ok())
                {
                    Some(cached) => slots[index] = Some(cached),
                    None => {
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
            let mut missing_offset = 0usize;
            for batch in missing_texts.chunks(TEXT_EMBEDDING_BATCH_SIZE) {
                self.acquire_embed_permit(self.estimate_tokens_for_texts(batch))
                    .await?;
                let batch_vectors = self.embed_openai_compatible_text(batch).await?;
                anyhow::ensure!(
                    batch_vectors.len() == batch.len(),
                    "embedding batch returned {} vectors for {} inputs",
                    batch_vectors.len(),
                    batch.len()
                );
                if let Some(cache) = &self.cache {
                    for (text, vector) in batch.iter().zip(batch_vectors.iter()) {
                        let key = embedding_cache_key(
                            &self.config.model,
                            self.config.dimensions,
                            &sha256_hex(text),
                        );
                        if let Ok(raw) = serde_json::to_string(vector) {
                            let _ = cache.set(&key, &raw, EMBEDDING_CACHE_TTL_SECS).await;
                        }
                    }
                }
                for (batch_index, vector) in batch_vectors.into_iter().enumerate() {
                    let original_index = missing_indices[missing_offset + batch_index];
                    slots[original_index] = Some(vector);
                }
                missing_offset += batch.len();
            }
        }

        let mut vectors = Vec::with_capacity(texts.len());
        for (index, slot) in slots.into_iter().enumerate() {
            let vector = slot.ok_or_else(|| {
                anyhow::anyhow!(
                    "embedding reassembly missing vector for input index {index} of {}",
                    texts.len()
                )
            })?;
            vectors.push(vector);
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

        let effective_dimension = dimension.or(self.config.dimensions);
        let cache_key = mm_embedding_cache_key(&self.config.model, effective_dimension, input);
        if let Some(cache) = &self.cache {
            if let Some(cached) = cache
                .get(&cache_key)
                .await
                .and_then(|raw| serde_json::from_str::<Vec<f32>>(&raw).ok())
            {
                return Ok(cached);
            }
        }

        let estimated_tokens = estimate_mm_tokens(input);
        self.acquire_embed_permit(estimated_tokens).await?;

        let (vector, actual_tokens_u32) = if self.uses_openai_vl_embedding() {
            self.embed_multimodal_fused_openai_vl(input, effective_dimension)
                .await?
        } else if self.uses_dashscope_multimodal_embedding() {
            self.embed_multimodal_fused_dashscope(input, effective_dimension)
                .await?
        } else {
            anyhow::bail!(
                "embed_multimodal_fused requires a DashScope or OpenAiVlEmbedding multimodal config"
            )
        };

        if let Some(limiter) = &self.rate_limiter {
            let actual_tokens = actual_tokens_u32
                .map(|value| value as usize)
                .unwrap_or(estimated_tokens);
            if actual_tokens > estimated_tokens {
                let _ = limiter.check_request(actual_tokens.saturating_sub(estimated_tokens));
            }
        }

        if let Some(cache) = &self.cache {
            if let Ok(raw) = serde_json::to_string(&vector) {
                let _ = cache.set(&cache_key, &raw, EMBEDDING_CACHE_TTL_SECS).await;
            }
        }

        self.record_embedding_usage(estimated_tokens as u32, actual_tokens_u32)
            .await;

        Ok(vector)
    }

    /// DashScope native multimodal embedding (`input.contents` object array +
    /// `parameters.{output_type,enable_fusion,dimension}`). Returns `(vector,
    /// actual_tokens)`.
    async fn embed_multimodal_fused_dashscope(
        &self,
        input: &MultiModalEmbeddingInput,
        effective_dimension: Option<usize>,
    ) -> anyhow::Result<(Vec<f32>, Option<u32>)> {
        let contents = build_dashscope_multimodal_contents(input);
        if contents.is_empty() {
            anyhow::bail!("multimodal embedding input is empty");
        }

        let mut parameters = serde_json::Map::new();
        parameters.insert("output_type".to_string(), json!("dense"));
        if contents.len() > 1 {
            parameters.insert("enable_fusion".to_string(), json!(true));
        }
        if let Some(dimension) = effective_dimension {
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

        let actual_tokens_u32 = resp
            .usage
            .as_ref()
            .and_then(|usage| usage.image_tokens.or(usage.total_tokens))
            .or_else(|| {
                resp.output
                    .embeddings
                    .first()
                    .and_then(|item| item.image_tokens)
            });

        let vector = resp
            .output
            .embeddings
            .into_iter()
            .next()
            .map(|item| item.embedding)
            .context("DashScope multimodal embedding response did not include any vectors")?;

        Ok((vector, actual_tokens_u32))
    }

    /// SiliconFlow Qwen3-VL-Embedding-8B: OpenAI-shaped `POST {base}/embeddings`
    /// with multimodal `input` object array (`[{image}, {text}]`) + `dimensions`.
    /// Reuses `build_dashscope_multimodal_contents` object shape for `input`.
    ///
    /// **SF does NOT fuse server-side**: a mixed list is batch input and the
    /// response carries one vector per item (verified 2026-08-03: `data[0]` ==
    /// image-only, `data[1]` == text-only, cos=1.0; official docs define no
    /// fusion semantics). Taking `data[0]` would silently drop the caption.
    /// Multi-part inputs are fused client-side (L2-normalize → mean →
    /// renormalize); single-part results pass through untouched (DashScope
    /// parity). Returns `(vector, actual_tokens)`.
    async fn embed_multimodal_fused_openai_vl(
        &self,
        input: &MultiModalEmbeddingInput,
        effective_dimension: Option<usize>,
    ) -> anyhow::Result<(Vec<f32>, Option<u32>)> {
        let contents = build_dashscope_multimodal_contents(input);
        if contents.is_empty() {
            anyhow::bail!("multimodal embedding input is empty");
        }

        let mut request_body = json!({
            "model": self.config.model,
            "input": contents,
        });
        if let Some(dimension) = effective_dimension {
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
            .context("Failed to send OpenAI-VL multimodal embedding request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "OpenAI-VL multimodal embedding API error {}: {}",
                status,
                body
            );
        }

        #[derive(Deserialize)]
        struct OpenAIEmbeddingResponse {
            data: Vec<OpenAIEmbeddingItem>,
        }
        #[derive(Deserialize)]
        struct OpenAIEmbeddingItem {
            embedding: Vec<f32>,
        }

        let resp: OpenAIEmbeddingResponse = response
            .json()
            .await
            .context("Failed to parse OpenAI-VL multimodal embedding response")?;

        let vectors: Vec<Vec<f32>> = resp.data.into_iter().map(|item| item.embedding).collect();
        anyhow::ensure!(
            !vectors.is_empty(),
            "OpenAI-VL multimodal embedding response did not include any vectors"
        );
        // No server-side fusion on SF: fuse per-item vectors client-side.
        Ok((fuse_part_vectors(&vectors)?, None))
    }

    fn uses_openai_vl_embedding(&self) -> bool {
        self.config.api_style == Some(crate::ApiStyle::OpenAiVlEmbedding)
    }

    fn uses_dashscope_multimodal_embedding(&self) -> bool {
        matches!(
            self.config.api_style,
            Some(crate::ApiStyle::DashScopeMultimodalEmbedding)
        ) || matches!(
            self.config.model.as_str(),
            "qwen3-vl-embedding"
                | "tongyi-embedding-vision-plus-2026-03-06"
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

        anyhow::ensure!(
            resp.data.len() == texts.len(),
            "embedding provider returned {} vectors for {} texts",
            resp.data.len(),
            texts.len()
        );

        let estimated = self.estimate_tokens_for_texts(texts) as u32;
        self.record_embedding_usage(estimated, None).await;

        Ok(resp.data.into_iter().map(|d| d.embedding).collect())
    }
}

#[async_trait::async_trait]
impl avrag_rag_core_ports::EmbeddingPort for EmbeddingClient {
    async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        EmbeddingClient::embed(self, texts).await
    }

    async fn embed_multimodal_fused(
        &self,
        input: &avrag_rag_core_ports::MultiModalEmbeddingInput,
        dimension: Option<usize>,
    ) -> anyhow::Result<Vec<f32>> {
        EmbeddingClient::embed_multimodal_fused(self, input, dimension).await
    }
}

#[cfg(test)]
mod tests;
