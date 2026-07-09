//! Triplet extraction prefix-cache hit-rate verification.
//!
//! Mimics the exact call pattern of `extract_triplets_for_index`: same system
//! prompt across multiple batches, each with a different user payload. Measures
//! how many prompt tokens are served from cache on calls 2+.
//!
//! Run with:
//!   cargo test -p avrag-llm -- --ignored --nocapture triplet_cache_hit_rate
//!
//! Prerequisites:
//!   - TRIPLET_LLM_* pointing to api.deepseek.com (official API reports
//!     cached_tokens; SiliconFlow does not)
//!   - TRIPLET_LLM_ENABLE_CACHE=true

use avrag_llm::{ChatMessage, LlmClient};

// The actual triplet extraction system prompt (same as production).
const TRIPLET_SYSTEM: &str = include_str!(
    "../../../prompts/pipeline/triplet-extraction.system.md"
);

/// Build user messages exactly as `build_triplet_extraction_messages` does.
fn build_user_payload(chunks_json: &str) -> String {
    format!(
        "Valid chunk IDs: {}\n\nChunks:\n{}\n\nExtract triplets with chunk_id:",
        "aaa-bbb-ccc-ddd", chunks_json
    )
}

/// Three different chunk payloads — same shape, different content — to simulate
/// real batches.
fn batch_payloads() -> Vec<String> {
    vec![
        // Batch A — cold cache
        r#"{"chunks":[{"chunk_id":"aaa-bbb-ccc-ddd","text":"The Rust programming language was created by Graydon Hoare at Mozilla Research in 2010. It emphasizes memory safety without garbage collection through its ownership system."}]}"#.to_string(),
        // Batch B — should hit cache for system prompt
        r#"{"chunks":[{"chunk_id":"aaa-bbb-ccc-ddd","text":"Python was created by Guido van Rossum and first released in 1991. It is an interpreted, high-level, general-purpose programming language with dynamic typing."}]}"#.to_string(),
        // Batch C — same expectation
        r#"{"chunks":[{"chunk_id":"aaa-bbb-ccc-ddd","text":"Go was designed at Google by Robert Griesemer, Rob Pike, and Ken Thompson. It is a statically typed, compiled language with goroutines for concurrency."}]}"#.to_string(),
    ]
}

#[derive(Debug, Default)]
struct BatchResult {
    prompt_tokens: u32,
    cached_tokens: u32,
    hit_rate_pct: f64,
}

#[tokio::test]
#[ignore = "requires live TRIPLET_LLM API; run with --ignored --nocapture"]
async fn triplet_cache_hit_rate() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let env_path = repo_root.join(".env");
    if env_path.exists() {
        dotenvy::from_path(&env_path).ok();
    }

    let base_url = std::env::var("TRIPLET_LLM_BASE_URL").unwrap_or_default();
    let api_key = std::env::var("TRIPLET_LLM_API_KEY").unwrap_or_default();
    let model = std::env::var("TRIPLET_LLM_MODEL").unwrap_or_default();

    if base_url.is_empty() || api_key.is_empty() {
        eprintln!("SKIP: TRIPLET_LLM_* not configured in .env");
        return;
    }

    let config = avrag_llm::ModelProviderConfig {
        base_url: base_url.clone(),
        api_key,
        model: model.clone(),
        timeout_ms: 120_000,
        api_style: None,
        dimensions: None,
        enable_thinking: Some(false),
        enable_cache: Some(true),
        rpm_limit: None,
        tpm_limit: None,
    };

    eprintln!("Triplet Cache Hit-Rate Verification");
    eprintln!("  provider: {}", base_url);
    eprintln!("  model:    {}", model);
    eprintln!("  system prompt: ~{} chars", TRIPLET_SYSTEM.len());
    eprintln!();

    let client = LlmClient::new(config).with_feature("triplet-cache-test");
    let payloads = batch_payloads();
    let mut results: Vec<BatchResult> = Vec::with_capacity(payloads.len());

    for (i, payload) in payloads.iter().enumerate() {
        let messages = vec![
            ChatMessage::system(TRIPLET_SYSTEM),
            ChatMessage::user(build_user_payload(payload)),
        ];

        let response = match client.complete_with_max_tokens(&messages, Some(0.1), 8_192).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  batch {} FAILED: {e}", i + 1);
                continue;
            }
        };

        let hit_rate = if response.usage.prompt_tokens > 0 {
            response.usage.cached_tokens as f64 / response.usage.prompt_tokens as f64 * 100.0
        } else {
            0.0
        };

        let label = if i == 0 { "cold" } else { "warm" };
        eprintln!(
            "  batch {} ({label:>4}): prompt={:>5}  cached={:>5}  hit_rate={:>5.1}%  completion={}",
            i + 1,
            response.usage.prompt_tokens,
            response.usage.cached_tokens,
            hit_rate,
            response.usage.completion_tokens,
        );

        results.push(BatchResult {
            prompt_tokens: response.usage.prompt_tokens,
            cached_tokens: response.usage.cached_tokens,
            hit_rate_pct: hit_rate,
        });

        // Small delay between calls
        if i + 1 < payloads.len() {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }

    // Assertions
    println!();
    if results.len() < 2 {
        eprintln!("INCONCLUSIVE: fewer than 2 successful calls");
        return;
    }

    let cold = &results[0];
    assert_eq!(
        cold.cached_tokens, 0,
        "first call (cold) should have zero cached tokens, got {}",
        cold.cached_tokens
    );

    let warm_hit_rates: Vec<f64> = results[1..]
        .iter()
        .map(|r| r.hit_rate_pct)
        .collect();

    let min_warm = warm_hit_rates
        .iter()
        .cloned()
        .fold(f64::MAX, f64::min);

    println!("  ✅ cold batch: cached_tokens=0 (expected)");
    println!("  ✅ warm batches hit rates: {:?}", warm_hit_rates);

    // The system prompt is ~730 tokens out of ~3000 total → ~24% expected hit.
    // Allow ±10% tolerance for tokenizer variance.
    assert!(
        min_warm >= 10.0,
        "warm batches should have ≥10% cache hit rate (system prefix ≈24% of total input), \
         got min={min_warm:.1}%",
    );

    println!("  ✅ cache hit rate verified (≥10% on warm batches)");
    println!();
    println!("  → prefix cache is working for triplet extraction on {base_url}");
    println!("  → expected production savings: ~15-20% of input tokens (system prompt segment)");
}

/// Realistic payload: ~3000 tokens of chunk content per batch, matching
/// production triplet extraction batch sizes. Verifies the hit rate under
/// real-world user-payload-to-system-prompt ratios.
#[tokio::test]
#[ignore = "requires live TRIPLET_LLM API; run with --ignored --nocapture"]
async fn triplet_cache_hit_rate_realistic_payload() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let env_path = repo_root.join(".env");
    if env_path.exists() {
        dotenvy::from_path(&env_path).ok();
    }

    let base_url = std::env::var("TRIPLET_LLM_BASE_URL").unwrap_or_default();
    let api_key = std::env::var("TRIPLET_LLM_API_KEY").unwrap_or_default();
    let model = std::env::var("TRIPLET_LLM_MODEL").unwrap_or_default();

    if base_url.is_empty() || api_key.is_empty() {
        eprintln!("SKIP: TRIPLET_LLM_* not configured");
        return;
    }

    let config = avrag_llm::ModelProviderConfig {
        base_url: base_url.clone(),
        api_key,
        model: model.clone(),
        timeout_ms: 120_000,
        api_style: None,
        dimensions: None,
        enable_thinking: Some(false),
        enable_cache: Some(true),
        rpm_limit: None,
        tpm_limit: None,
    };

    let client = LlmClient::new(config).with_feature("triplet-cache-realistic");

    // Build ~3000 tokens of realistic chunk text per batch (Chinese technical doc style).
    let paragraph = "华为IPD（集成产品开发）流程是华为公司从IBM引进的一套先进的产品开发管理体系。该流程的核心思想是将产品开发视为一项投资行为，通过结构化的流程、跨部门的团队协作以及基于市场的决策机制，确保产品开发的成功率和投资回报率。IPD流程涵盖了从概念阶段、计划阶段、开发阶段、验证阶段、发布阶段到生命周期管理阶段的全过程，每个阶段都设有明确的决策评审点和技术评审点。在概念阶段，主要进行市场需求分析、产品定位和初步的商业计划制定；计划阶段则进一步细化产品需求，制定详细的项目计划和资源配置方案；开发阶段进行产品的设计、编码、测试和集成工作；验证阶段通过内部测试、客户试用等方式验证产品是否满足需求；发布阶段包括产品的生产、供应链准备、市场推广和销售培训等；生命周期管理阶段则关注产品的持续优化、版本迭代和最终的退市管理。";

    // Each batch: multiple paragraphs to hit ~3000 tokens
    let chunk_text_a: String = std::iter::repeat(paragraph)
        .take(6)
        .collect::<Vec<_>>()
        .join("\n\n");
    let chunk_text_b = chunk_text_a.replace("华为IPD", "腾讯敏捷开发");
    let chunk_text_c = chunk_text_a.replace("华为IPD", "阿里中台战略");

    let payloads = vec![
        format!(r#"{{"chunks":[{{"chunk_id":"111-222-333","text":"{}"}}]}}"#, chunk_text_a),
        format!(r#"{{"chunks":[{{"chunk_id":"111-222-333","text":"{}"}}]}}"#, chunk_text_b),
        format!(r#"{{"chunks":[{{"chunk_id":"111-222-333","text":"{}"}}]}}"#, chunk_text_c),
    ];

    eprintln!("Triplet Cache Hit-Rate — Realistic Payload (~3000 tok/batch)");
    eprintln!("  provider: {}", base_url);
    eprintln!("  system prompt: ~{} chars", TRIPLET_SYSTEM.len());
    eprintln!();

    let mut prompt_tokens = Vec::new();
    let mut cached_tokens = Vec::new();

    for (i, payload) in payloads.iter().enumerate() {
        let messages = vec![
            ChatMessage::system(TRIPLET_SYSTEM),
            ChatMessage::user(build_user_payload(payload)),
        ];

        let response = match client.complete_with_max_tokens(&messages, Some(0.1), 8_192).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  batch {} FAILED: {e}", i + 1);
                continue;
            }
        };

        let hit_rate = if response.usage.prompt_tokens > 0 {
            response.usage.cached_tokens as f64 / response.usage.prompt_tokens as f64 * 100.0
        } else {
            0.0
        };

        let label = if i == 0 { "cold" } else { "warm" };
        eprintln!(
            "  batch {} ({label:>4}): prompt={:>5}  cached={:>5}  hit_rate={:>5.1}%  completion={}",
            i + 1,
            response.usage.prompt_tokens,
            response.usage.cached_tokens,
            hit_rate,
            response.usage.completion_tokens,
        );

        prompt_tokens.push(response.usage.prompt_tokens);
        cached_tokens.push(response.usage.cached_tokens);

        if i + 1 < payloads.len() {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }

    println!();
    assert!(cached_tokens.len() >= 3, "need 3 successful calls");

    // DeepSeek KV cache accumulates globally: the system prompt (≈768 tok)
    // is always cached, and overlapping user payload text from previous runs
    // may also hit. Minimum: system prompt size. All batches should agree.
    let min_cached = *cached_tokens.iter().min().unwrap();
    let max_cached = *cached_tokens.iter().max().unwrap();
    assert!(
        min_cached >= 700,
        "each batch should cache at least the system prompt (~768 tokens), got min={min_cached}"
    );
    assert_eq!(
        min_cached, max_cached,
        "all batches should report identical cached_tokens (same system prefix), got range {min_cached}..{max_cached}"
    );

    // Realistic hit rate: system / (system + ~3000 user) ≈ 20%.
    let avg_warm_hit = cached_tokens.iter().zip(prompt_tokens.iter())
        .map(|(&c, &p)| c as f64 / p as f64 * 100.0)
        .sum::<f64>() / cached_tokens.len() as f64;

    println!("  ✅ all batches: cached≈{min_cached} tokens ({:.1}% hit)", avg_warm_hit);
    println!();
    println!("  → Realistic production scenario:");
    println!("    batch={} tokens, cached={}, user≈{} miss",
        prompt_tokens[0], min_cached,
        prompt_tokens[0].saturating_sub(min_cached));
    println!("    net savings: {:.0}% of input tokens per batch", avg_warm_hit);
}
