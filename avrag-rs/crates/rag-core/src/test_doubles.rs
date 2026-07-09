//! Shared test doubles for rag-core (no avrag-llm dependency).

use std::sync::Arc;

use async_trait::async_trait;
use avrag_rag_core_ports::{
    EmbeddingPort, MultiModalEmbeddingInput, MultiModalRerankDocument, RerankPort, RerankResult,
};

use crate::RagConfig;

/// Deterministic embedding double for unit tests.
#[derive(Default)]
pub struct StubEmbeddingPort;

#[async_trait]
impl EmbeddingPort for StubEmbeddingPort {
    async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        Ok(texts
            .iter()
            .map(|t| {
                let mut v = vec![0.1_f32; 8];
                if let Some(b) = t.as_bytes().first() {
                    v[0] = (*b as f32) / 255.0;
                }
                v
            })
            .collect())
    }

    async fn embed_multimodal_fused(
        &self,
        input: &MultiModalEmbeddingInput,
        _dimension: Option<usize>,
    ) -> anyhow::Result<Vec<f32>> {
        let mut v = vec![0.2_f32; 8];
        if let Some(text) = input.text.as_deref() {
            if let Some(b) = text.as_bytes().first() {
                v[0] = (*b as f32) / 255.0;
            }
        }
        Ok(v)
    }
}

/// No-op reranker that preserves input order.
#[derive(Default)]
pub struct StubRerankPort;

#[async_trait]
impl RerankPort for StubRerankPort {
    async fn rerank(
        &self,
        _query: &str,
        documents: &[&str],
    ) -> anyhow::Result<Vec<RerankResult>> {
        Ok(documents
            .iter()
            .enumerate()
            .map(|(index, _)| RerankResult {
                index,
                score: 1.0 - (index as f32) * 0.01,
            })
            .collect())
    }

    async fn rerank_multimodal_text_query(
        &self,
        _query: &str,
        documents: &[MultiModalRerankDocument],
        top_n: usize,
    ) -> anyhow::Result<Vec<RerankResult>> {
        Ok(documents
            .iter()
            .enumerate()
            .take(top_n.max(1))
            .map(|(index, _)| RerankResult {
                index,
                score: 1.0 - (index as f32) * 0.01,
            })
            .collect())
    }
}

pub fn stub_embedding() -> Arc<dyn EmbeddingPort> {
    Arc::new(StubEmbeddingPort) as Arc<dyn EmbeddingPort>
}

pub fn test_rag_config() -> RagConfig {
    RagConfig::new_for_data_plane(stub_embedding(), None)
}
