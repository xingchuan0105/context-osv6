use avrag_llm::{ChatMessage, LlmResponse};
use tokio_util::sync::CancellationToken;

/// 安全的 LLM 服务 trait
/// 实现内部持有 API key，但不暴露给调用方
pub trait LlmService: Send + Sync {
    fn complete(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
        token: CancellationToken,
    ) -> impl std::future::Future<Output = anyhow::Result<LlmResponse>> + Send;

    fn provider_name(&self) -> String;
    fn model_name(&self) -> String;
}

/// 安全的 Embedding 服务 trait
pub trait EmbeddingService: Send + Sync {
    fn embed(
        &self,
        text: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<Vec<f32>>> + Send;
    fn embed_batch(
        &self,
        texts: &[String],
    ) -> impl std::future::Future<Output = anyhow::Result<Vec<Vec<f32>>>> + Send;
}

/// 安全的搜索服务 trait
pub trait SearchService: Send + Sync {
    fn search(
        &self,
        query: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<Vec<SearchResult>>> + Send;
    fn provider(&self) -> String;
    fn mode(&self) -> String;
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// 安全的存储服务 trait
pub trait StorageService: Send + Sync {
    fn generate_upload_url(
        &self,
        document_id: &str,
        object_path: &str,
        expires_secs: u64,
    ) -> impl std::future::Future<Output = anyhow::Result<String>> + Send;
    fn generate_download_url(
        &self,
        object_path: &str,
        expires_secs: u64,
    ) -> impl std::future::Future<Output = anyhow::Result<String>> + Send;
}

/// 服务配置（非敏感子集）
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub public_base_url: String,
    pub object_root: String,
    pub usage_limit_phase: String,
    pub search_provider: String,
    pub search_mode: String,
    pub object_storage_upload_expire_sec: u64,
    pub object_storage_download_expire_sec: u64,
}
