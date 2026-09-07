use std::sync::Arc;

use async_trait::async_trait;
use subtex_store_sqlite::{GlobalStore, SubtexStore};

/// Embedding seam. The production impl calls the cloud client; tests plug in
/// deterministic embedders so the vector layer is exercised without network.
#[async_trait]
pub trait TextEmbedder: Send + Sync {
    async fn embed(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingSettings {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout_ms: u64,
}

impl EmbeddingSettings {
    /// Same env chain the B-line uses (`EMBEDDING_*` with a
    /// `DASHSCOPE_API_KEY` fallback), so no new credential entry appears.
    pub fn from_env() -> Option<Self> {
        let api_key = non_empty_env("EMBEDDING_API_KEY").or_else(|| non_empty_env("DASHSCOPE_API_KEY"))?;
        Some(Self {
            base_url: non_empty_env("EMBEDDING_BASE_URL")
                .unwrap_or_else(|| "https://api.siliconflow.cn/v1".to_string()),
            api_key,
            model: non_empty_env("EMBEDDING_MODEL").unwrap_or_else(|| "Pro/BAAI/bge-m3".to_string()),
            timeout_ms: non_empty_env("EMBEDDING_TIMEOUT_MS")
                .and_then(|v| v.parse().ok())
                .unwrap_or(60_000),
        })
    }
}

fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

pub struct CloudEmbedder {
    client: avrag_llm::EmbeddingClient,
}

impl CloudEmbedder {
    pub fn from_settings(settings: &EmbeddingSettings, usage_sink: Option<Arc<StoreUsageObserver>>) -> Self {
        let config = avrag_llm::ModelProviderConfig {
            base_url: settings.base_url.clone(),
            api_key: settings.api_key.clone(),
            model: settings.model.clone(),
            timeout_ms: settings.timeout_ms,
            api_style: None,
            // bge-m3 style providers reject a `dimensions` field; the store's
            // 1024-dim layout matches the configured model.
            dimensions: None,
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        };
        let mut client = avrag_llm::EmbeddingClient::new(config);
        if let Some(observer) = usage_sink {
            client = client.with_observer(
                observer as Arc<dyn avrag_llm::UsageObserver>,
                avrag_llm::TenantContext::new(uuid::Uuid::nil(), uuid::Uuid::nil()),
            );
        }
        Self { client }
    }

    /// Ready-to-use cloud embedder recording every call into the store's
    /// usage ledger (and the global credits book when provided).
    pub fn with_store_ledger(store: Arc<SubtexStore>) -> Option<Self> {
        Self::with_ledgers(store, None)
    }

    pub fn with_ledgers(store: Arc<SubtexStore>, global: Option<Arc<GlobalStore>>) -> Option<Self> {
        EmbeddingSettings::from_env().map(|settings| {
            Self::from_settings(
                &settings,
                Some(Arc::new(StoreUsageObserver { store, global })),
            )
        })
    }
}

#[async_trait]
impl TextEmbedder for CloudEmbedder {
    async fn embed(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        self.client.embed(&refs).await
    }
}

/// Usage-observer adapter: every embedding call lands in the per-root usage
/// table and, when a global store is present, the F7 millicredit ledger.
pub struct StoreUsageObserver {
    store: Arc<SubtexStore>,
    global: Option<Arc<GlobalStore>>,
}

#[async_trait]
impl avrag_llm::UsageObserver for StoreUsageObserver {
    async fn record_chat(&self, _tenant: &avrag_llm::TenantContext, _record: &avrag_llm::ChatUsageRecord) {
    }

    async fn record_embedding(
        &self,
        _tenant: &avrag_llm::TenantContext,
        record: &avrag_llm::EmbeddingUsageRecord,
    ) {
        let tokens = record
            .actual_tokens
            .map(f64::from)
            .unwrap_or(f64::from(record.estimated_tokens));
        let _ = self
            .store
            .record_usage("embedding", Some(&record.model), tokens, Some("tokens"), None);
        if let Some(global) = &self.global {
            let _ = global.record_credit(
                "embedding",
                Some(&record.model),
                tokens,
                Some("tokens"),
                None,
            );
        }
    }
}
