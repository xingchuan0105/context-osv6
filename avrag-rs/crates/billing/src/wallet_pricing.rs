//! Platform-proxy list pricing for wallet usage debits (ADR-0010 §3, §8).
//!
//! ```text
//! list_fen = ceil(official_cost_fen * LIST_PRICE_MULTIPLIER)
//! LIST_PRICE_MULTIPLIER = 1.5   // markup 50% / ~33% gross margin
//! ```
//!
//! Official rates are fen (分) per 1_000_000 tokens for a small whitelist of
//! platform-proxy models (DeepSeek flash / Qwen flash / SiliconFlow embed).
//!
//! Default: code constants. Ops override via `PLATFORM_OFFICIAL_RATES_JSON`:
//! ```json
//! [{"provider":"deepseek","model_contains":"flash","input":100,"cache":2,"output":200}]
//! ```

use std::sync::OnceLock;
use uuid::Uuid;

/// Explicit list-price multiplier: official × 1.5 (ADR-0010 §3.1 / §7.4).
pub const LIST_PRICE_MULTIPLIER: f64 = 1.5;

/// Official unit rates in fen (分) per 1_000_000 tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OfficialRates {
    /// Cache-miss / ordinary input tokens.
    pub input_fen_per_mtok: f64,
    /// Prompt-cache hit tokens (0 when the model has no cache pricing).
    pub cache_fen_per_mtok: f64,
    /// Completion / output tokens (0 for pure embedding models).
    pub output_fen_per_mtok: f64,
}

/// Default / DeepSeek-flash-class rates (placeholder official CNY-ish fen).
const RATES_DEEPSEEK_FLASH: OfficialRates = OfficialRates {
    input_fen_per_mtok: 100.0, // ¥1 / 1M
    cache_fen_per_mtok: 2.0,   // ¥0.02 / 1M
    output_fen_per_mtok: 200.0, // ¥2 / 1M
};

const RATES_DEEPSEEK_PRO: OfficialRates = OfficialRates {
    input_fen_per_mtok: 200.0,
    cache_fen_per_mtok: 4.0,
    output_fen_per_mtok: 400.0,
};

const RATES_QWEN_FLASH: OfficialRates = OfficialRates {
    input_fen_per_mtok: 50.0,
    cache_fen_per_mtok: 5.0,
    output_fen_per_mtok: 100.0,
};

const RATES_SILICONFLOW_EMBED: OfficialRates = OfficialRates {
    input_fen_per_mtok: 10.0,
    cache_fen_per_mtok: 0.0,
    output_fen_per_mtok: 0.0,
};

#[derive(Debug, Clone, serde::Deserialize)]
struct EnvRateRow {
    provider: Option<String>,
    model_contains: String,
    input: f64,
    #[serde(default)]
    cache: f64,
    #[serde(default)]
    output: f64,
}

fn env_rate_overrides() -> &'static [EnvRateRow] {
    static ROWS: OnceLock<Vec<EnvRateRow>> = OnceLock::new();
    ROWS.get_or_init(|| {
        let raw = std::env::var("PLATFORM_OFFICIAL_RATES_JSON").unwrap_or_default();
        if raw.trim().is_empty() {
            return Vec::new();
        }
        serde_json::from_str(&raw).unwrap_or_else(|e| {
            eprintln!("PLATFORM_OFFICIAL_RATES_JSON parse failed; using code defaults: {e}");
            Vec::new()
        })
    })
}

/// Resolve official fen/1M rates for a **whitelist** provider+model pair.
///
/// ADR-0010 §3.1: unknown models return `None` (do not silently price as flash).
/// Match is by stable substrings; marketing names can change — ops should align
/// env model ids with these patterns or extend this table deliberately.
/// Optional `PLATFORM_OFFICIAL_RATES_JSON` overrides take precedence.
pub fn official_rates_for(provider: &str, model: &str) -> Option<OfficialRates> {
    let p = provider.trim().to_ascii_lowercase();
    let m = model.trim().to_ascii_lowercase();

    for row in env_rate_overrides() {
        let prov_ok = row
            .provider
            .as_ref()
            .map(|x| p.contains(&x.to_ascii_lowercase()))
            .unwrap_or(true);
        if prov_ok && m.contains(&row.model_contains.to_ascii_lowercase()) {
            return Some(OfficialRates {
                input_fen_per_mtok: row.input,
                cache_fen_per_mtok: row.cache,
                output_fen_per_mtok: row.output,
            });
        }
    }

    // Embedding whitelist (SiliconFlow / names containing embed/bge).
    if m.contains("embed") || m.contains("bge-m3") || m.contains("bge_m3") {
        return Some(RATES_SILICONFLOW_EMBED);
    }
    if p.contains("siliconflow") && (m.contains("bge") || m.contains("qwen")) {
        if !m.contains("instruct") && !m.contains("chat") && !m.contains("turbo") {
            return Some(RATES_SILICONFLOW_EMBED);
        }
    }

    // Qwen flash tier (avoid hardcoding a single marketing revision).
    if m.contains("qwen") && m.contains("flash") {
        return Some(RATES_QWEN_FLASH);
    }

    // DeepSeek flash / pro tiers.
    if p == "deepseek" || m.contains("deepseek") {
        if m.contains("flash") {
            return Some(RATES_DEEPSEEK_FLASH);
        }
        if m.contains("chat") || m.contains("reasoner") || m.contains("pro") {
            return Some(RATES_DEEPSEEK_PRO);
        }
        // Known deepseek without flash/pro: refuse rather than invent a tier.
        return None;
    }

    None
}

/// List price in fen: `ceil(official * 1.5)`.
///
/// - `0` when both token counts are zero.
/// - `None` when the model is **not** on the platform-proxy whitelist (caller must not bill silently).
pub fn list_price_fen(
    provider: &str,
    model: &str,
    prompt_tokens: u32,
    completion_tokens: u32,
    cached_tokens: u32,
) -> Option<i64> {
    if prompt_tokens == 0 && completion_tokens == 0 {
        return Some(0);
    }
    let rates = official_rates_for(provider, model)?;
    let cached = (cached_tokens.min(prompt_tokens)) as f64;
    let miss = (prompt_tokens as f64) - cached;
    let out = completion_tokens as f64;
    let official = miss / 1_000_000.0 * rates.input_fen_per_mtok
        + cached / 1_000_000.0 * rates.cache_fen_per_mtok
        + out / 1_000_000.0 * rates.output_fen_per_mtok;
    let list = (official * LIST_PRICE_MULTIPLIER).ceil();
    if list.is_finite() && list > 0.0 {
        Some(list as i64)
    } else {
        Some(0)
    }
}

/// Stable idempotency key for a platform-proxy usage debit.
///
/// Prefer a real usage-event UUID when available; otherwise a request-scoped
/// or synthetic key so `apply_ledger_entry` replays are safe.
pub fn usage_debit_idempotency_key(event_id: Uuid) -> String {
    format!("usage_debit:{event_id}")
}

/// Build an idempotency key from a request id when no DB event id is available.
pub fn usage_debit_idempotency_key_for_request(user_id: Uuid, request_id: &str) -> String {
    format!("usage_debit:req:{user_id}:{request_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_price_multiplier_is_explicit_1_5() {
        assert!((LIST_PRICE_MULTIPLIER - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn one_million_input_deepseek_flash_is_150_fen() {
        // official 100 fen → ×1.5 = 150
        assert_eq!(
            list_price_fen("deepseek", "deepseek-v4-flash", 1_000_000, 0, 0),
            Some(150)
        );
    }

    #[test]
    fn one_million_output_deepseek_flash_is_300_fen() {
        // official 200 fen → ×1.5 = 300
        assert_eq!(
            list_price_fen("deepseek", "deepseek-v4-flash", 0, 1_000_000, 0),
            Some(300)
        );
    }

    #[test]
    fn small_call_ceils_to_at_least_one_fen() {
        // 1k in + 1k out flash: official = 0.1 + 0.2 = 0.3 → *1.5 = 0.45 → ceil 1
        assert_eq!(
            list_price_fen("deepseek", "deepseek-v4-flash", 1000, 1000, 0),
            Some(1)
        );
    }

    #[test]
    fn zero_tokens_zero_fen() {
        assert_eq!(
            list_price_fen("deepseek", "deepseek-v4-flash", 0, 0, 0),
            Some(0)
        );
    }

    #[test]
    fn unknown_model_not_whitelisted() {
        assert_eq!(list_price_fen("openai", "gpt-4o", 1000, 1000, 0), None);
    }

    #[test]
    fn qwen_flash_and_embed_use_whitelist_rates() {
        assert_eq!(
            list_price_fen("dashscope", "qwen3.5-flash", 1_000_000, 0, 0),
            Some(75) // 50 * 1.5
        );
        assert_eq!(
            list_price_fen(
                "siliconflow",
                "Qwen/Qwen3-Embedding-8B",
                1_000_000,
                0,
                0
            ),
            Some(15) // 10 * 1.5
        );
    }

    #[test]
    fn cache_hit_cheaper_than_miss() {
        let miss = list_price_fen("deepseek", "deepseek-v4-flash", 1_000_000, 0, 0).unwrap();
        let hit =
            list_price_fen("deepseek", "deepseek-v4-flash", 1_000_000, 0, 1_000_000).unwrap();
        assert!(hit < miss);
        // full cache hit: official 2 fen → *1.5 = 3
        assert_eq!(hit, 3);
    }

    #[test]
    fn idempotency_key_formats() {
        let id = Uuid::nil();
        assert_eq!(
            usage_debit_idempotency_key(id),
            "usage_debit:00000000-0000-0000-0000-000000000000"
        );
        assert_eq!(
            usage_debit_idempotency_key_for_request(id, "req-1"),
            "usage_debit:req:00000000-0000-0000-0000-000000000000:req-1"
        );
    }
}
