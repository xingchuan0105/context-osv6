//! LLM 配置：`INGESTION_LLM_*` env → `ModelProviderConfig`（对齐 `supervise.env_cfg`）。

use avrag_llm::{LlmClient, ModelProviderConfig};

/// 从 env 构造 LlmClient（INGESTION_LLM_BASE_URL / INGESTION_LLM_API_KEY /
/// INGESTION_LLM_MODEL；缺失时返回 None，调用方决定降级）。
/// `INGESTION_LLM_ENABLE_CACHE=true` 开启 dashscope 显式上下文缓存标记
/// （窗口化摘要会话的 system 前缀与逐窗口增长链可跨请求命中）。
pub fn llm_client_from_env() -> Option<LlmClient> {
    let base_url = std::env::var("INGESTION_LLM_BASE_URL").ok()?;
    let api_key = std::env::var("INGESTION_LLM_API_KEY").ok()?;
    let model = std::env::var("INGESTION_LLM_MODEL").unwrap_or_else(|_| "deepseek-v4-flash".into());
    let enable_cache = std::env::var("INGESTION_LLM_ENABLE_CACHE")
        .ok()
        .map(|v| matches!(v.trim(), "1" | "true" | "yes"));
    Some(LlmClient::new(ModelProviderConfig {
        base_url,
        api_key,
        model,
        timeout_ms: 120_000,
        api_style: None,
        dimensions: None,
        enable_thinking: None,
        enable_cache,
        rpm_limit: None,
        tpm_limit: None,
    }))
}
