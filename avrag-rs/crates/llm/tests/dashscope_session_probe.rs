//! 真实 API 探针：DashScope Responses 会话（previous_response_id 链 +
//! `x-dashscope-session-cache`）两轮调用，验证 Rust 路径的
//! `response_id` 往返与 `usage.input_tokens_details.cached_tokens` 解析。
//!
//! 运行（默认 `#[ignore]`，需真实 key）：
//! ```bash
//! cd avrag-rs && set -a && source .env && set +a && \
//!   cargo test -p avrag-llm --test dashscope_session_probe -- --ignored --nocapture
//! ```

use avrag_llm::{ApiStyle, ChatMessage, LlmClient, ModelProviderConfig};

fn env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

/// 首轮塞 >1024 token 的前缀（会话缓存最小门槛），续接轮观察 cached_tokens。
#[tokio::test]
#[ignore = "real DashScope API; run explicitly with INGESTION_LLM_* env"]
async fn dashscope_session_cache_probe() {
    let config = ModelProviderConfig {
        base_url: env("INGESTION_LLM_BASE_URL").expect("INGESTION_LLM_BASE_URL"),
        api_key: env("INGESTION_LLM_API_KEY").expect("INGESTION_LLM_API_KEY"),
        model: env("INGESTION_LLM_MODEL").unwrap_or_else(|| "qwen3.7-flash".into()),
        timeout_ms: 60_000,
        api_style: Some(ApiStyle::DashScopeResponses),
        dimensions: None,
        enable_thinking: Some(false),
        enable_cache: None,
        rpm_limit: None,
        tpm_limit: None,
    };
    let client = LlmClient::new(config);

    // ~1.5k token 的确定性前缀（重复段落，避免依赖外部文件）。
    let para = "生物质锅炉采用两台4T/H与一台3T/H并联供汽，制粒车间配置粉碎机、烘干机与制粒机各四台。";
    let mut prefix = String::new();
    for i in 0..60 {
        prefix.push_str(&format!("[段{i:02}]{para}"));
    }
    assert!(prefix.len() > 3000, "seed prefix should exceed cache minimum");

    let turn1 = vec![
        ChatMessage::system("你是文档摘要助手。"),
        ChatMessage::user(format!("文档如下：\n{prefix}\n\n用一句话概括文档主题。")),
    ];
    let (resp1, id1) = client
        .complete_response(None, &turn1, Some(0.1))
        .await
        .expect("turn1 seed");
    eprintln!(
        "turn1: response_id={:?} prompt={} completion={} cached={}",
        id1,
        resp1.usage.prompt_tokens,
        resp1.usage.completion_tokens,
        resp1.usage.cached_tokens
    );
    let id1 = id1.expect("turn1 must return a response id for chaining");

    // 续接轮与 seed 轮 instructions 必须完全一致（DashScope 会话缓存键含
    // instructions；生产 session 载体已按此约束把 instructions 恒定为
    // INTERACTION_SESSION_SYSTEM，阶段指令折叠进 user 消息）。
    let turn2 = vec![
        ChatMessage::system("你是文档摘要助手。"),
        ChatMessage::user("文档中 4T/H 的锅炉有几台？只回答数字。".to_string()),
    ];
    let (resp2, id2) = client
        .complete_response(Some(&id1), &turn2, Some(0.1))
        .await
        .expect("turn2 continuation");
    eprintln!(
        "turn2: response_id={:?} prompt={} completion={} cached={} answer={:?}",
        id2,
        resp2.usage.prompt_tokens,
        resp2.usage.completion_tokens,
        resp2.usage.cached_tokens,
        resp2.content.chars().take(40).collect::<String>()
    );

    assert!(id2.is_some(), "turn2 must also return a response id");
    // 会话缓存命中：续接轮的 cached_tokens 应显著大于 0（前缀 >1024 token）。
    assert!(
        resp2.usage.cached_tokens > 0,
        "expected session-cache hit on continuation turn, got cached_tokens=0 \
         (prompt={}, 检查 x-dashscope-session-cache 头与 previous_response_id 是否生效)",
        resp2.usage.prompt_tokens
    );
}
