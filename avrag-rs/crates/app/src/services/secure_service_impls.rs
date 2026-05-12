use tokio_util::sync::CancellationToken;
use crate::services::secure_services::{EmbeddingService, LlmService, SearchResult, SearchService, StorageService};
use avrag_llm::{ChatMessage, LlmClient, LlmResponse};
use std::sync::Arc;

/// 安全的 LLM 服务实现
/// 内部持有 LlmClient（包含 API key），但不暴露
pub struct SecureLlmService {
    inner: LlmClient,
    provider: String,
    model: String,
}

impl SecureLlmService {
    pub fn new(inner: LlmClient) -> Self {
        let provider = inner.config.provider_name();
        let model = inner.config.model.clone();
        Self {
            inner,
            provider,
            model,
        }
    }
}

impl LlmService for SecureLlmService {
    fn complete(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
        _token: CancellationToken,
    ) -> impl std::future::Future<Output = anyhow::Result<LlmResponse>> + Send {
        self.inner.complete(messages, temperature)
    }

    fn provider_name(&self) -> String {
        self.provider.clone()
    }

    fn model_name(&self) -> String {
        self.model.clone()
    }
}

/// 安全的 Embedding 服务实现
pub struct SecureEmbeddingService {
    inner: Arc<avrag_llm::EmbeddingClient>,
}

impl SecureEmbeddingService {
    pub fn new(inner: Arc<avrag_llm::EmbeddingClient>) -> Self {
        Self { inner }
    }
}

impl EmbeddingService for SecureEmbeddingService {
    fn embed(
        &self,
        text: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<Vec<f32>>> + Send {
        let inner = self.inner.clone();
        let text = text.to_string();
        async move {
            let results = inner.embed(&[&text]).await?;
            results.into_iter().next().ok_or_else(|| anyhow::anyhow!("empty embedding result"))
        }
    }

    fn embed_batch(
        &self,
        texts: &[String],
    ) -> impl std::future::Future<Output = anyhow::Result<Vec<Vec<f32>>>> + Send {
        let inner = self.inner.clone();
        let texts: Vec<String> = texts.to_vec();
        async move {
            let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
            inner.embed(&refs).await
        }
    }
}

/// 安全的搜索服务实现
pub struct SecureSearchService {
    _inner: Arc<avrag_search::SearchExecutor>,
    provider: String,
    mode: String,
}

impl SecureSearchService {
    pub fn new(inner: Arc<avrag_search::SearchExecutor>, provider: String, mode: String) -> Self {
        Self {
            _inner: inner,
            provider,
            mode,
        }
    }
}

impl SearchService for SecureSearchService {
    fn search(
        &self,
        _query: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<Vec<SearchResult>>> + Send {
        std::future::ready(Ok(vec![]))
    }

    fn provider(&self) -> String {
        self.provider.clone()
    }

    fn mode(&self) -> String {
        self.mode.clone()
    }
}

/// 安全的存储服务实现
pub struct SecureStorageService {
    _object_store: Arc<avrag_storage_pg::ObjectStoreHandle>,
    public_base_url: String,
}

impl SecureStorageService {
    pub fn new(
        object_store: Arc<avrag_storage_pg::ObjectStoreHandle>,
        public_base_url: String,
    ) -> Self {
        Self {
            _object_store: object_store,
            public_base_url,
        }
    }
}

impl StorageService for SecureStorageService {
    fn generate_upload_url(
        &self,
        document_id: &str,
        _object_path: &str,
        expires_secs: u64,
    ) -> impl std::future::Future<Output = anyhow::Result<String>> + Send {
        let public_base_url = self.public_base_url.clone();
        let document_id = document_id.to_string();
        std::future::ready(Ok(format!(
            "{}/uploads/{}?expires={}",
            public_base_url, document_id, expires_secs
        )))
    }

    fn generate_download_url(
        &self,
        object_path: &str,
        expires_secs: u64,
    ) -> impl std::future::Future<Output = anyhow::Result<String>> + Send {
        let public_base_url = self.public_base_url.clone();
        let object_path = object_path.to_string();
        std::future::ready(Ok(format!(
            "{}/download/{}?expires={}",
            public_base_url, object_path, expires_secs
        )))
    }
}
