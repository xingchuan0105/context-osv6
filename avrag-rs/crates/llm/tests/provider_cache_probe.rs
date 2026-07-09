//! Provider prefix-cache probe — one-shot diagnostic to determine whether
//! each LLM provider returns `cached_tokens` in its usage block for repeated
//! identical prefix queries.
//!
//! Run with:
//!   cargo test -p avrag-llm -- --ignored --nocapture provider_cache_probe
//!
//! Requires a valid `.env` in the repo root with `E2E_LLM_*` (DeepSeek official)
//! and `TRIPLET_LLM_*` (SiliconFlow) credentials.

use avrag_llm::{ChatMessage, LlmClient};
use std::io::Write;

/// A long enough system prompt to exceed the minimum prefix length for KV cache.
const LONG_SYSTEM_PROMPT: &str = "\
You are a precise, factual assistant. Your task is to analyze and repeat back \
information from the provided text. You must follow these instructions carefully:

1. Read the entire input text thoroughly.
2. Identify the key entities, relationships, and facts mentioned.
3. Organize the information into a structured format.
4. Return only the structured output, with no additional commentary.

The input format will be a passage of text describing a technical architecture. \
You must extract system components and their responsibilities, data flows between \
components, external dependencies and interfaces, and performance characteristics.

For each component, provide: name, type (service, database, queue, cache, gateway), \
responsibilities (2-5 items), dependencies, interfaces (APIs, protocols, ports).

For data flows, describe: source, destination, data format (JSON, protobuf, binary), \
throughput characteristics, latency requirements.

Output format: JSON with { \"components\": [...], \"data_flows\": [...] } structure.

You are a helpful, harmless, and honest assistant. Always answer in a concise and \
accurate manner. Do not make up information. If you don't know something, say so. \
Respond in the same language as the user's query. Use proper formatting when \
presenting structured data. Be respectful and professional at all times. Avoid \
controversial topics unless directly relevant to the query. Provide citations or \
references when making factual claims. When providing code, use proper syntax \
highlighting and explain the key parts. Break down complex problems into manageable \
steps. Verify your reasoning before presenting conclusions. Consider edge cases and \
potential pitfalls in your analysis. When uncertain, clearly indicate your confidence.";

#[derive(Debug)]
struct ProbeResult {
    provider: String,
    base_url: String,
    model: String,
    call1_prompt_tokens: u32,
    call1_cached_tokens: u32,
    call2_prompt_tokens: u32,
    call2_cached_tokens: u32,
    cache_works: bool,
    error: Option<String>,
}

async fn probe_provider(name: &str, prefix: &str) -> ProbeResult {
    let base_url = std::env::var(format!("{prefix}_BASE_URL")).unwrap_or_default();
    let api_key = std::env::var(format!("{prefix}_API_KEY")).unwrap_or_default();
    let model = std::env::var(format!("{prefix}_MODEL")).unwrap_or_default();

    if base_url.is_empty() || api_key.is_empty() {
        return ProbeResult {
            provider: name.to_string(),
            base_url,
            model,
            call1_prompt_tokens: 0,
            call1_cached_tokens: 0,
            call2_prompt_tokens: 0,
            call2_cached_tokens: 0,
            cache_works: false,
            error: Some("missing credentials".to_string()),
        };
    }

    let config = avrag_llm::ModelProviderConfig {
        base_url: base_url.clone(),
        api_key,
        model: model.clone(),
        timeout_ms: 60_000,
        api_style: None,
        dimensions: None,
        enable_thinking: Some(false),
        enable_cache: Some(true),
        rpm_limit: None,
        tpm_limit: None,
    };

    let client = LlmClient::new(config).with_feature("probe");

    let messages = vec![
        ChatMessage::system(LONG_SYSTEM_PROMPT),
        ChatMessage::user("Repeat: The quick brown fox jumps over the lazy dog."),
    ];

    // Call 1 — fills the cache
    let result1 = match client.complete(&messages, Some(0.0)).await {
        Ok(resp) => resp,
        Err(e) => {
            return ProbeResult {
                provider: name.to_string(),
                base_url,
                model,
                call1_prompt_tokens: 0,
                call1_cached_tokens: 0,
                call2_prompt_tokens: 0,
                call2_cached_tokens: 0,
                cache_works: false,
                error: Some(format!("call 1 failed: {e}")),
            };
        }
    };

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Call 2 — should hit cache if provider supports it
    let result2 = match client.complete(&messages, Some(0.0)).await {
        Ok(resp) => resp,
        Err(e) => {
            return ProbeResult {
                provider: name.to_string(),
                base_url,
                model,
                call1_prompt_tokens: result1.usage.prompt_tokens,
                call1_cached_tokens: result1.usage.cached_tokens,
                call2_prompt_tokens: 0,
                call2_cached_tokens: 0,
                cache_works: false,
                error: Some(format!("call 2 failed: {e}")),
            };
        }
    };

    let cache_works = result2.usage.cached_tokens > 0;

    ProbeResult {
        provider: name.to_string(),
        base_url,
        model,
        call1_prompt_tokens: result1.usage.prompt_tokens,
        call1_cached_tokens: result1.usage.cached_tokens,
        call2_prompt_tokens: result2.usage.prompt_tokens,
        call2_cached_tokens: result2.usage.cached_tokens,
        cache_works,
        error: None,
    }
}

fn print_table(results: &[ProbeResult]) {
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout);
    let _ = writeln!(stdout, "Provider Prefix Cache Probe Results");
    let _ = writeln!(stdout, "====================================");
    let _ = writeln!(
        stdout,
        "{:<15} {:<55} {:<25} {:>8} {:>8} {:>8} {:>8}  {:>6}",
        "Provider", "Base URL", "Model",
        "C1-Prompt", "C1-Cached", "C2-Prompt", "C2-Cached", "Works?",
    );
    let _ = writeln!(stdout, "{:-<150}", "");
    for r in results {
        let works = if r.cache_works { "YES" } else { "no" };
        let err = r.error.as_deref().unwrap_or("");
        let _ = writeln!(
            stdout,
            "{:<15} {:<55} {:<25} {:>8} {:>8} {:>8} {:>8}  {:>6}  {}",
            r.provider, r.base_url, r.model,
            r.call1_prompt_tokens, r.call1_cached_tokens,
            r.call2_prompt_tokens, r.call2_cached_tokens,
            works, err,
        );
    }
    let _ = writeln!(stdout);
}

#[tokio::test]
#[ignore = "requires live API credentials from .env; run with --ignored --nocapture"]
async fn probe_provider_prefix_cache() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let env_path = repo_root.join(".env");
    if env_path.exists() {
        dotenvy::from_path(&env_path).ok();
    }

    let mut results = Vec::new();

    if std::env::var("E2E_LLM_API_KEY").is_ok() {
        results.push(probe_provider("deepseek-official", "E2E_LLM").await);
    } else {
        eprintln!("[SKIP] deepseek-official: E2E_LLM_* not configured");
    }

    if std::env::var("TRIPLET_LLM_API_KEY").is_ok() {
        // Label derived from base_url via the canonical provider inference so
        // it never mismatches when the env prefix is repointed to a different
        // provider.
        let base_url = std::env::var("TRIPLET_LLM_BASE_URL").unwrap_or_default();
        let kind = avrag_llm::ProviderKind::from_base_url(&base_url);
        let label = match kind {
            avrag_llm::ProviderKind::DeepSeek => "deepseek",
            avrag_llm::ProviderKind::SiliconFlow => "siliconflow",
            _ => "triplet-provider",
        };
        results.push(probe_provider(label, "TRIPLET_LLM").await);
    } else {
        eprintln!("[SKIP] triplet-provider: TRIPLET_LLM_* not configured");
    }

    print_table(&results);

    for r in &results {
        if r.error.is_none() && !r.cache_works {
            eprintln!(
                "WARNING: {} ({}) did NOT report cached tokens on second call.",
                r.provider, r.base_url,
            );
        }
    }
}
