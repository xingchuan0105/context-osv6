//! Embedding / rerank / planner ports so rag-core does not hard-depend on avrag-llm.

use async_trait::async_trait;
use contracts::chat::RagPlan;

use crate::llm_types::LlmUsage;

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
}

#[derive(Debug, Clone)]
pub enum MultiModalRerankDocument {
    Text(String),
    Image(String),
    Video(String),
}

#[derive(Debug, Clone)]
pub struct RerankResult {
    pub index: usize,
    pub score: f32,
}

#[async_trait]
pub trait EmbeddingPort: Send + Sync {
    async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>>;

    async fn embed_multimodal_fused(
        &self,
        input: &MultiModalEmbeddingInput,
        dimension: Option<usize>,
    ) -> anyhow::Result<Vec<f32>>;
}

#[async_trait]
pub trait RerankPort: Send + Sync {
    async fn rerank(&self, query: &str, documents: &[&str]) -> anyhow::Result<Vec<RerankResult>>;

    async fn rerank_multimodal_text_query(
        &self,
        query: &str,
        documents: &[MultiModalRerankDocument],
        top_n: usize,
    ) -> anyhow::Result<Vec<RerankResult>>;
}

/// Optional retrieval planner (legacy / test surfaces).
#[async_trait]
pub trait PlannerPort: Send + Sync {
    async fn plan_with_usage(
        &self,
        query: &str,
        session_context: Option<&str>,
        docscope: Option<&common::DocScopeMetadata>,
    ) -> anyhow::Result<(RagPlan, LlmUsage)>;
}
