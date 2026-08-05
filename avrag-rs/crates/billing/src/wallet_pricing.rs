//! Platform-proxy list pricing for wallet usage debits (ADR-0010 §3, §8).
//!
//! ```text
//! list_fen = ceil(official_cost_fen * LIST_PRICE_MULTIPLIER)
//! LIST_PRICE_MULTIPLIER = 1.5   // markup 50% / ~33% gross margin
//! ```
//!
//! Official rates are fen (分) per 1_000_000 tokens for a small whitelist of
//! platform-proxy models (DeepSeek flash / Qwen flash / SiliconFlow embed).
//! v1 keeps rates as code constants — no DB price table.

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

/// Resolve official fen/1M rates for a provider+model pair (whitelist + default).
pub fn official_rates_for(provider: &str, model: &str) -> OfficialRates {
    let p = provider.trim().to_ascii_lowercase();
    let m = model.trim().to_ascii_lowercase();

    // Embedding whitelist (SiliconFlow / generic embed models).
    if m.contains("embed") {
        return RATES_SILICONFLOW_EMBED;
    }
    if p.contains("siliconflow") && (m.contains("bge") || m.contains("qwen")) {
        // Common SF embedding model ids without "embed" in the name.
        if !m.contains("instruct") && !m.contains("chat") {
            return RATES_SILICONFLOW_EMBED;
        }
    }

    // Qwen flash tier.
    if m.contains("qwen") && m.contains("flash") {
        return RATES_QWEN_FLASH;
    }

    // DeepSeek tiers.
    if p == "deepseek" || m.contains("deepseek") {
        if m.contains("flash") {
            return RATES_DEEPSEEK_FLASH;
        }
        return RATES_DEEPSEEK_PRO;
    }

    // Default: treat unknown platform-proxy models as flash-class.
    RATES_DEEPSEEK_FLASH
}

/// List price in fen for a usage event: `ceil(official * 1.5)`.
///
/// Returns `0` when both token counts are zero (nothing to bill).
/// Fractional fen below 1 still ceil to at least 1 when official × 1.5 > 0.
pub fn list_price_fen(
    provider: &str,
    model: &str,
    prompt_tokens: u32,
    completion_tokens: u32,
    cached_tokens: u32,
) -> i64 {
    if prompt_tokens == 0 && completion_tokens == 0 {
        return 0;
    }
    let rates = official_rates_for(provider, model);
    let cached = (cached_tokens.min(prompt_tokens)) as f64;
    let miss = (prompt_tokens as f64) - cached;
    let out = completion_tokens as f64;
    let official = miss / 1_000_000.0 * rates.input_fen_per_mtok
        + cached / 1_000_000.0 * rates.cache_fen_per_mtok
        + out / 1_000_000.0 * rates.output_fen_per_mtok;
    let list = (official * LIST_PRICE_MULTIPLIER).ceil();
    if list.is_finite() && list > 0.0 {
        list as i64
    } else {
        0
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
            150
        );
    }

    #[test]
    fn one_million_output_deepseek_flash_is_300_fen() {
        // official 200 fen → ×1.5 = 300
        assert_eq!(
            list_price_fen("deepseek", "deepseek-v4-flash", 0, 1_000_000, 0),
            300
        );
    }

    #[test]
    fn small_call_ceils_to_at_least_one_fen() {
        // 1k in + 1k out flash: official = 0.1 + 0.2 = 0.3 → *1.5 = 0.45 → ceil 1
        assert_eq!(
            list_price_fen("deepseek", "deepseek-v4-flash", 1000, 1000, 0),
            1
        );
    }

    #[test]
    fn zero_tokens_zero_fen() {
        assert_eq!(list_price_fen("deepseek", "deepseek-v4-flash", 0, 0, 0), 0);
    }

    #[test]
    fn qwen_flash_and_embed_use_whitelist_rates() {
        assert_eq!(
            list_price_fen("dashscope", "qwen3.5-flash", 1_000_000, 0, 0),
            75 // 50 * 1.5
        );
        assert_eq!(
            list_price_fen(
                "siliconflow",
                "Qwen/Qwen3-Embedding-8B",
                1_000_000,
                0,
                0
            ),
            15 // 10 * 1.5
        );
    }

    #[test]
    fn cache_hit_cheaper_than_miss() {
        let miss = list_price_fen("deepseek", "deepseek-v4-flash", 1_000_000, 0, 0);
        let hit = list_price_fen("deepseek", "deepseek-v4-flash", 1_000_000, 0, 1_000_000);
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
