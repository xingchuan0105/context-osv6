//! Platform-proxy list pricing for wallet usage debits (ADR-0010 §3, §8).
//!
//! ```text
//! list_fen = ceil(official_cost_fen * LIST_PRICE_MULTIPLIER)
//! LIST_PRICE_MULTIPLIER = 1.5   // markup 50% / ~33% gross margin
//! ```
//!
//! Official unit rates are **configuration, not code**: they come from
//! `PLATFORM_OFFICIAL_RATES_JSON` (fen / 分 per 1_000_000 tokens). A model with
//! no matching row is not billable — `None` (callers fail-open unpaid + log).
//!
//! Row shapes (first matching row wins; match = optional `provider` substring
//! + `model_contains` substring, both case-insensitive):
//!
//! ```json
//! [
//!   {"model_contains":"v4-flash",
//!    "peak":{"input":300,"cache":10,"output":900},
//!    "off_peak":{"input":150,"cache":5,"output":450}},
//!   {"model_contains":"qwen3.7-flash",
//!    "tiers":[
//!      {"max_prompt_tokens":32000,"input":20,"cache":4,"output":80},
//!      {"max_prompt_tokens":256000,"input":60,"cache":12,"output":240},
//!      {"max_prompt_tokens":1000000,"input":120,"cache":24,"output":480}]},
//!   {"model_contains":"bge-m3","input":7}
//! ]
//! ```
//!
//! - flat row: `input` (+ optional `cache` / `output`) — e.g. embedding models.
//! - peak/off-peak row: `peak` + `off_peak` rate sets, selected by the
//!   Beijing-time peak windows below (DeepSeek 2026-08-17 peak/off-peak notice).
//! - tiered row: `tiers` selected by prompt tokens — first tier whose
//!   `max_prompt_tokens` covers the prompt; prompts above the last tier bill at
//!   the last tier's rates.

use chrono::{DateTime, FixedOffset, Timelike, Utc};
use uuid::Uuid;

/// Explicit list-price multiplier: official × 1.5 (ADR-0010 §3.1 / §7.4).
pub const LIST_PRICE_MULTIPLIER: f64 = 1.5;

/// DeepSeek peak windows in Beijing time (UTC+8): 09:00–12:00, 14:00–18:00
/// (vendor peak/off-peak pricing effective 2026-08-17; off-peak is half price).
const PEAK_WINDOWS_BEIJING: &[(u32, u32)] = &[(9, 12), (14, 18)];

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

#[derive(Debug, Clone, Copy, serde::Deserialize)]
struct RateSet {
    input: f64,
    #[serde(default)]
    cache: f64,
    #[serde(default)]
    output: f64,
}

impl From<RateSet> for OfficialRates {
    fn from(s: RateSet) -> Self {
        OfficialRates {
            input_fen_per_mtok: s.input,
            cache_fen_per_mtok: s.cache,
            output_fen_per_mtok: s.output,
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
struct TierRate {
    max_prompt_tokens: u32,
    input: f64,
    #[serde(default)]
    cache: f64,
    #[serde(default)]
    output: f64,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct RateRow {
    provider: Option<String>,
    model_contains: String,
    // Flat form (embedding etc.).
    input: Option<f64>,
    #[serde(default)]
    cache: f64,
    #[serde(default)]
    output: f64,
    // Peak/off-peak form (DeepSeek).
    peak: Option<RateSet>,
    off_peak: Option<RateSet>,
    // Tiered-by-prompt-length form (Qwen flash).
    tiers: Option<Vec<TierRate>>,
}

impl RateRow {
    fn matches(&self, provider_lower: &str, model_lower: &str) -> bool {
        let prov_ok = self
            .provider
            .as_ref()
            .map(|x| provider_lower.contains(&x.to_ascii_lowercase()))
            .unwrap_or(true);
        prov_ok && model_lower.contains(&self.model_contains.to_ascii_lowercase())
    }

    /// Rate-set selection — THE single structural truth. Both `rates()` and
    /// `all_rate_sets()` derive from this one match, so a row shape the
    /// runtime cannot serve (lone peak/off-peak, empty tiers, a flat rate
    /// missing alongside a lone peak) is invisible to every consumer alike
    /// (review round-5 S1: the gate and runtime had drifted on hybrid rows).
    ///
    /// Returns `None` when the shape is unservable at runtime; `Some` carries
    /// every rate set the shape declares.
    fn rate_sets(&self) -> Option<Vec<RateSet>> {
        let mut sets: Vec<RateSet> = Vec::new();
        if let Some(tiers) = &self.tiers {
            if tiers.is_empty() {
                return None;
            }
            sets.extend(tiers.iter().map(|t| RateSet {
                input: t.input,
                cache: t.cache,
                output: t.output,
            }));
            return Some(sets);
        }
        match (&self.peak, &self.off_peak) {
            (Some(peak), Some(off_peak)) => {
                sets.push(*peak);
                sets.push(*off_peak);
            }
            // A flat input alongside a lone peak/off-peak is ambiguous — the
            // runtime would have to pick one arbitrarily, so the shape is
            // unservable and must resolve like any other broken row.
            (Some(_), None) | (None, Some(_)) => return None,
            (None, None) => {}
        }
        if sets.is_empty() {
            // Flat form.
            sets.push(RateSet {
                input: self.input?,
                cache: self.cache,
                output: self.output,
            });
        }
        Some(sets)
    }

    /// Every rate set the row declares — the startup gate validates all of
    /// them. Returns `None` exactly when `rates()` would return `None`.
    fn all_rate_sets(&self) -> Option<Vec<RateSet>> {
        self.rate_sets()
    }

    fn billable_set(set: &RateSet) -> bool {
        set.input.is_finite() && set.input > 0.0 && set.cache >= 0.0 && set.output >= 0.0
    }

    /// Rates for a concrete debit — the ONLY runtime path. It selects a rate
    /// set from the same shape the gate validated: tier by prompt length, or
    /// peak/off-peak by time, or the flat set. `None` for unservable shapes.
    fn rates(&self, prompt_tokens: u32, at: DateTime<Utc>) -> Option<OfficialRates> {
        let sets = self.rate_sets()?;
        if self.tiers.is_some() {
            let tier = self
                .tiers
                .as_ref()
                .and_then(|tiers| {
                    tiers
                        .iter()
                        .find(|t| prompt_tokens <= t.max_prompt_tokens)
                        .or(tiers.last())
                })?;
            return Some(
                RateSet {
                    input: tier.input,
                    cache: tier.cache,
                    output: tier.output,
                }
                .into(),
            );
        }
        if self.peak.is_some() && self.off_peak.is_some() {
            let set = if is_beijing_peak(at) {
                &sets[0]
            } else {
                &sets[1]
            };
            return Some((*set).into());
        }
        Some(sets[0].into())
    }

    /// Representative rates for whitelist checks: first tier / peak / flat.
    fn representative(&self) -> Option<OfficialRates> {
        if let Some(tiers) = &self.tiers {
            if let Some(tier) = tiers.first() {
                return Some(
                    RateSet {
                        input: tier.input,
                        cache: tier.cache,
                        output: tier.output,
                    }
                    .into(),
                );
            }
        }
        if let Some(peak) = &self.peak {
            return Some((*peak).into());
        }
        self.rates(0, Utc::now())
    }
}

/// Parse configured rate rows from `PLATFORM_OFFICIAL_RATES_JSON`.
///
/// Read per call (not cached) so tests and long-running processes see the
/// current env value; empty/unparseable → no rows (nothing is billable).
fn configured_rate_rows() -> Vec<RateRow> {
    let raw = std::env::var("PLATFORM_OFFICIAL_RATES_JSON").unwrap_or_default();
    if raw.trim().is_empty() {
        return Vec::new();
    }
    serde_json::from_str(&raw).unwrap_or_else(|e| {
        eprintln!("PLATFORM_OFFICIAL_RATES_JSON parse failed; no platform rates configured: {e}");
        Vec::new()
    })
}

/// Beijing (UTC+8) peak-window check for peak/off-peak rows.
fn is_beijing_peak(at: DateTime<Utc>) -> bool {
    let beijing = at.with_timezone(&FixedOffset::east_opt(8 * 3600).expect("+08:00 is valid"));
    let hour = beijing.hour();
    PEAK_WINDOWS_BEIJING
        .iter()
        .any(|&(start, end)| hour >= start && hour < end)
}

/// First configured row matching provider+model that resolves to rates.
fn resolve_in(
    rows: &[RateRow],
    provider: &str,
    model: &str,
    prompt_tokens: u32,
    at: DateTime<Utc>,
) -> Option<OfficialRates> {
    let p = provider.trim().to_ascii_lowercase();
    let m = model.trim().to_ascii_lowercase();
    rows.iter()
        .filter(|r| r.matches(&p, &m))
        .find_map(|r| r.rates(prompt_tokens, at))
}

/// Whitelist check used by relay / startup validation: representative
/// configured rates for a provider+model pair (`None` = not billable).
pub fn official_rates_for(provider: &str, model: &str) -> Option<OfficialRates> {
    let p = provider.trim().to_ascii_lowercase();
    let m = model.trim().to_ascii_lowercase();
    configured_rate_rows()
        .iter()
        .filter(|r| r.matches(&p, &m))
        .find_map(|r| r.representative())
}

fn price_from_rates(
    rates: &OfficialRates,
    prompt_tokens: u32,
    completion_tokens: u32,
    cached_tokens: u32,
) -> i64 {
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

/// List price in fen: `ceil(official * 1.5)`, official rates from
/// `PLATFORM_OFFICIAL_RATES_JSON` at the current instant.
///
/// - `0` when both token counts are zero.
/// - `None` when the model matches **no** configured rate row (caller must not
///   bill silently).
pub fn list_price_fen(
    provider: &str,
    model: &str,
    prompt_tokens: u32,
    completion_tokens: u32,
    cached_tokens: u32,
) -> Option<i64> {
    list_price_fen_at(
        provider,
        model,
        prompt_tokens,
        completion_tokens,
        cached_tokens,
        Utc::now(),
    )
}

/// Startup price-gate helper (chat-first W3 §8.5): true when the FIRST row
/// matching `provider` + `model` — the row runtime resolution would pick — is
/// servable AND billable (every declared rate set has a positive input and
/// non-negative cache/output; review round-4 P0: `need_fen <= 0` skips the
/// hold so a non-positive rate free-rides; review round-6: gate and runtime
/// share ONE shape resolver — `all_rate_sets()` returns `None` exactly when
/// `rates()` does, so hybrid/broken rows cannot boot). Pure — takes the raw
/// rate JSON so callers can gate without touching the env.
pub fn rates_present_in(raw: &str, provider: &str, model: &str) -> bool {
    if raw.trim().is_empty() {
        return false;
    }
    let Ok(rows) = serde_json::from_str::<Vec<RateRow>>(raw) else {
        return false;
    };
    let p = provider.trim().to_ascii_lowercase();
    let m = model.trim().to_ascii_lowercase();
    rows.iter()
        .filter(|r| r.matches(&p, &m))
        .find_map(|r| {
            r.all_rate_sets()
                .map(|sets| sets.iter().all(RateRow::billable_set))
        })
        .unwrap_or(false)
}

fn list_price_fen_at(
    provider: &str,
    model: &str,
    prompt_tokens: u32,
    completion_tokens: u32,
    cached_tokens: u32,
    at: DateTime<Utc>,
) -> Option<i64> {
    if prompt_tokens == 0 && completion_tokens == 0 {
        return Some(0);
    }
    let rates = resolve_in(&configured_rate_rows(), provider, model, prompt_tokens, at)?;
    Some(price_from_rates(
        &rates,
        prompt_tokens,
        completion_tokens,
        cached_tokens,
    ))
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
    use chrono::TimeZone;

    /// Mirror of the ops JSON shapes (values = the 2026-08-17 vendor prices).
    fn test_rows() -> Vec<RateRow> {
        serde_json::from_str(r#"[
            {"model_contains":"v4-flash",
             "peak":{"input":300,"cache":10,"output":900},
             "off_peak":{"input":150,"cache":5,"output":450}},
            {"model_contains":"v4-pro",
             "peak":{"input":900,"cache":30,"output":2700},
             "off_peak":{"input":450,"cache":15,"output":1350}},
            {"model_contains":"qwen3.7-flash",
             "tiers":[
               {"max_prompt_tokens":32000,"input":20,"cache":4,"output":80},
               {"max_prompt_tokens":256000,"input":60,"cache":12,"output":240},
               {"max_prompt_tokens":1000000,"input":120,"cache":24,"output":480}]},
            {"model_contains":"bge-m3","input":7},
            {"model_contains":"bge-reranker","input":7}
        ]"#)
        .unwrap()
    }

    /// 2026-08-17 10:00 Beijing (UTC+8) = 02:00 UTC — inside 09:00–12:00 peak.
    fn peak_time() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 17, 2, 0, 0).unwrap()
    }

    /// 2026-08-17 20:00 Beijing = 12:00 UTC — off-peak.
    fn off_peak_time() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 17, 12, 0, 0).unwrap()
    }

    fn price(
        rows: &[RateRow],
        provider: &str,
        model: &str,
        prompt: u32,
        completion: u32,
        cached: u32,
        at: DateTime<Utc>,
    ) -> Option<i64> {
        resolve_in(rows, provider, model, prompt, at)
            .map(|r| price_from_rates(&r, prompt, completion, cached))
    }

    #[test]
    fn list_price_multiplier_is_explicit_1_5() {
        assert!((LIST_PRICE_MULTIPLIER - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn deepseek_flash_peak_and_off_peak() {
        let rows = test_rows();
        let m = "deepseek-ai/DeepSeek-V4-Flash";
        // peak: 1M input miss 300 → ×1.5 = 450; 1M output 900 → 1350
        assert_eq!(price(&rows, "deepseek", m, 1_000_000, 0, 0, peak_time()), Some(450));
        assert_eq!(price(&rows, "deepseek", m, 0, 1_000_000, 0, peak_time()), Some(1350));
        // off-peak is half: 150 → 225; 450 → 675
        assert_eq!(
            price(&rows, "deepseek", m, 1_000_000, 0, 0, off_peak_time()),
            Some(225)
        );
        assert_eq!(
            price(&rows, "deepseek", m, 0, 1_000_000, 0, off_peak_time()),
            Some(675)
        );
    }

    #[test]
    fn deepseek_pro_peak_and_off_peak() {
        let rows = test_rows();
        // peak: input 900 → 1350; output 2700 → 4050; off-peak input 450 → 675
        assert_eq!(
            price(&rows, "deepseek", "deepseek-v4-pro", 1_000_000, 0, 0, peak_time()),
            Some(1350)
        );
        assert_eq!(
            price(&rows, "deepseek", "deepseek-v4-pro", 0, 1_000_000, 0, peak_time()),
            Some(4050)
        );
        assert_eq!(
            price(&rows, "deepseek", "deepseek-v4-pro", 1_000_000, 0, 0, off_peak_time()),
            Some(675)
        );
    }

    #[test]
    fn beijing_peak_window_boundaries() {
        let at = |h_utc: u32, min: u32| Utc.with_ymd_and_hms(2026, 8, 17, h_utc, min, 0).unwrap();
        assert!(!is_beijing_peak(at(0, 59))); // 08:59 Beijing
        assert!(is_beijing_peak(at(1, 0))); // 09:00
        assert!(is_beijing_peak(at(3, 59))); // 11:59
        assert!(!is_beijing_peak(at(4, 0))); // 12:00
        assert!(is_beijing_peak(at(6, 0))); // 14:00
        assert!(is_beijing_peak(at(9, 59))); // 17:59
        assert!(!is_beijing_peak(at(10, 0))); // 18:00
    }

    #[test]
    fn deepseek_cache_hit_cheaper_than_miss() {
        let rows = test_rows();
        let m = "deepseek-v4-flash";
        let miss = price(&rows, "deepseek", m, 1_000_000, 0, 0, peak_time()).unwrap();
        let hit = price(&rows, "deepseek", m, 1_000_000, 0, 1_000_000, peak_time()).unwrap();
        assert!(hit < miss);
        // full cache hit peak: 10 → ×1.5 = 15; off-peak: 5 → 7.5 → ceil 8
        assert_eq!(hit, 15);
        assert_eq!(
            price(&rows, "deepseek", m, 1_000_000, 0, 1_000_000, off_peak_time()),
            Some(8)
        );
    }

    #[test]
    fn small_call_ceils_to_at_least_one_fen() {
        let rows = test_rows();
        // peak: 1k in + 1k out flash = 0.3 + 0.9 = 1.2 official → ×1.5 = 1.8 → 2
        assert_eq!(
            price(&rows, "deepseek", "deepseek-v4-flash", 1000, 1000, 0, peak_time()),
            Some(2)
        );
        // off-peak: 0.15 + 0.45 = 0.6 → ×1.5 = 0.9 → 1
        assert_eq!(
            price(&rows, "deepseek", "deepseek-v4-flash", 1000, 1000, 0, off_peak_time()),
            Some(1)
        );
    }

    #[test]
    fn qwen_flash_tiers_by_prompt_tokens() {
        let rows = test_rows();
        let t = peak_time(); // time-independent row; any instant works
        // ≤32k tier: 32_000 in → 0.64 official → ×1.5 = 0.96 → 1
        assert_eq!(price(&rows, "dashscope", "qwen3.7-flash", 32_000, 0, 0, t), Some(1));
        // >32k tier: 32_001 in → ≈1.92 → ×1.5 ≈ 2.88 → 3
        assert_eq!(price(&rows, "dashscope", "qwen3.7-flash", 32_001, 0, 0, t), Some(3));
        // >256k tier: 1M in → 120 → 180; above the last tier bills at that tier
        assert_eq!(
            price(&rows, "dashscope", "qwen3.7-flash", 1_000_000, 0, 0, t),
            Some(180)
        );
        assert_eq!(
            price(&rows, "dashscope", "qwen3.7-flash", 2_000_000, 0, 0, t),
            Some(360)
        );
        // output-only call lands in the first tier: 1M out → 80 → 120
        assert_eq!(
            price(&rows, "dashscope", "qwen3.7-flash", 0, 1_000_000, 0, t),
            Some(120)
        );
        // cache hits use the tier's cache rate: 1M cached at top tier → 24 → 36
        assert_eq!(
            price(&rows, "dashscope", "qwen3.7-flash", 1_000_000, 0, 1_000_000, t),
            Some(36)
        );
    }

    #[test]
    fn embed_and_reranker_flat_rates() {
        let rows = test_rows();
        // ¥0.070 / 1M = 7 fen → ×1.5 = 10.5 → ceil 11
        assert_eq!(
            price(&rows, "siliconflow", "Pro/BAAI/bge-m3", 1_000_000, 0, 0, peak_time()),
            Some(11)
        );
        assert_eq!(
            price(
                &rows,
                "siliconflow",
                "Pro/BAAI/bge-reranker-v2-m3",
                1_000_000,
                0,
                0,
                peak_time()
            ),
            Some(11)
        );
    }

    #[test]
    fn unconfigured_model_is_not_billable() {
        let rows = test_rows();
        assert_eq!(
            price(&rows, "openai", "gpt-4o", 1000, 1000, 0, peak_time()),
            None
        );
        // No rows at all → nothing billable.
        assert_eq!(
            price(&[], "deepseek", "deepseek-v4-flash", 1000, 1000, 0, peak_time()),
            None
        );
    }

    #[test]
    fn first_matching_row_with_rates_wins() {
        // A shapeless row does not shadow a later row that carries rates.
        let rows: Vec<RateRow> = serde_json::from_str(r#"[
            {"model_contains":"bge-m3"},
            {"model_contains":"bge-m3","input":7}
        ]"#)
        .unwrap();
        assert_eq!(
            price(&rows, "siliconflow", "bge-m3", 1_000_000, 0, 0, peak_time()),
            Some(11)
        );
    }

    #[test]
    fn zero_tokens_zero_fen_without_any_config() {
        // Early return before config lookup.
        assert_eq!(list_price_fen("deepseek", "anything", 0, 0, 0), Some(0));
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

#[cfg(test)]
mod price_gate_tests {
    use super::*;
    use chrono::TimeZone;

    /// Review round-3: the missing-price startup gate relies on exact
    /// provider+model matching — cover the pure matcher here.
    #[test]
    fn rates_present_in_matches_provider_and_model() {
        let rows = r#"[{"model_contains":"qwen3.8-flash","input":20,"output":80}]"#;
        assert!(rates_present_in(rows, "dashscope", "qwen3.8-flash"));
        assert!(!rates_present_in(rows, "dashscope", "qwen3.7-flash"));
        assert!(!rates_present_in("", "dashscope", "qwen3.8-flash"));
        assert!(!rates_present_in("not json", "dashscope", "qwen3.8-flash"));
    }

    /// Review round-4 P0: a matching row with a zero or negative rate set is
    /// NOT a price — `need_fen <= 0` skips the hold, so the gate must reject
    /// every rate set the row can serve (flat, any tier, peak, off-peak).
    #[test]
    fn rates_present_in_rejects_non_billable_rates() {
        let zero = r#"[{"model_contains":"qwen3.8-flash","input":0,"output":0}]"#;
        let zero_input_positive_output = r#"[{"model_contains":"qwen3.8-flash","input":0,"output":80}]"#;
        let negative = r#"[{"model_contains":"qwen3.8-flash","input":-20,"output":80}]"#;
        let zero_tier = r#"[{"model_contains":"qwen3.8-flash","tiers":[
            {"max_prompt_tokens":32000,"input":0,"cache":0,"output":0},
            {"max_prompt_tokens":256000,"input":60,"cache":12,"output":240}]}]"#;
        let negative_cache_tier = r#"[{"model_contains":"qwen3.8-flash","tiers":[
            {"max_prompt_tokens":32000,"input":20,"cache":-4,"output":80}]}]"#;
        let zero_peak = r#"[{"model_contains":"v4-flash",
            "peak":{"input":0,"cache":0,"output":0},
            "off_peak":{"input":150,"cache":5,"output":450}}]"#;
        let negative_off_peak = r#"[{"model_contains":"v4-flash",
            "peak":{"input":300,"cache":10,"output":900},
            "off_peak":{"input":-150,"cache":5,"output":450}}]"#;
        // Lone peak/off-peak or empty tiers: shapes the runtime `rates()`
        // cannot serve — the gate must reject them too (review round-5 S1).
        let lone_peak = r#"[{"model_contains":"v4-flash",
            "peak":{"input":300,"cache":10,"output":900}}]"#;
        let empty_tiers = r#"[{"model_contains":"qwen3.8-flash","tiers":[]}]"#;
        for bad in [
            zero,
            zero_input_positive_output,
            negative,
            zero_tier,
            negative_cache_tier,
        ] {
            assert!(
                !rates_present_in(bad, "dashscope", "qwen3.8-flash"),
                "non-billable row must fail the gate: {bad}"
            );
        }
        for bad in [zero_peak, negative_off_peak, lone_peak] {
            assert!(
                !rates_present_in(bad, "deepseek", "v4-flash"),
                "peak/off-peak row must fail the gate: {bad}"
            );
        }
        assert!(!rates_present_in(empty_tiers, "dashscope", "qwen3.8-flash"));
    }

    /// Review round-5 S1: the gate picks the SAME row runtime resolution
    /// picks — the first match. A broken row placed before a good row must
    /// FAIL the gate (boot refuses), not succeed on the later good row.
    #[test]
    fn gate_resolves_first_matching_row_like_runtime() {
        // Runtime `resolve_in` would pick the first (zero) row and fail every
        // QuickChat hold after a successful boot — so the gate must refuse.
        let broken_first_rows = r#"[
            {"model_contains":"qwen3.8-flash","input":0,"output":0},
            {"model_contains":"qwen3.8-flash","input":20,"output":80}]"#;
        assert!(!rates_present_in(
            broken_first_rows,
            "dashscope",
            "qwen3.8-flash"
        ));

        // Reverse order: the good row is first — boot allowed.
        let good_first = r#"[
            {"model_contains":"qwen3.8-flash","input":20,"output":80},
            {"model_contains":"qwen3.8-flash","input":0,"output":0}]"#;
        assert!(rates_present_in(good_first, "dashscope", "qwen3.8-flash"));
    }

    /// Review round-5 (Spec-8): the matcher is provider-scoped — a row
    /// carrying a provider clause must not satisfy a different provider.
    #[test]
    fn rates_present_in_respects_provider_scope() {
        let rows = r#"[{"provider":"dashscope","model_contains":"qwen3.8-flash","input":20,"output":80}]"#;
        assert!(rates_present_in(rows, "dashscope", "qwen3.8-flash"));
        assert!(
            !rates_present_in(rows, "deepseek", "qwen3.8-flash"),
            "wrong provider must not pass"
        );
        // Provider-less row stays a wildcard (existing ops rows rely on it).
        let wildcard = r#"[{"model_contains":"qwen3.8-flash","input":20}]"#;
        assert!(rates_present_in(wildcard, "any-provider", "qwen3.8-flash"));
    }

    /// Review round-6 S1/Spec-1: gate and runtime share ONE shape resolver —
    /// a hybrid row (flat input + lone peak) previously priced at runtime via
    /// the flat rate while the gate skipped it. Both must now treat it as
    /// unservable: no pricing, no boot.
    #[test]
    fn hybrid_flat_plus_lone_peak_is_unservable_for_gate_and_runtime() {
        let rows: Vec<RateRow> = serde_json::from_str(
            r#"[{"model_contains":"hybrid","input":20,"output":80,
                 "peak":{"input":300,"cache":10,"output":900}}]"#,
        )
        .unwrap();
        let at = Utc.with_ymd_and_hms(2026, 8, 17, 2, 0, 0).unwrap();
        assert_eq!(rows[0].rates(1000, at), None, "runtime cannot price it");
        assert!(
            !rates_present_in(
                r#"[{"model_contains":"hybrid","input":20,"output":80,
                     "peak":{"input":300,"cache":10,"output":900}}]"#,
                "any",
                "hybrid"
            ),
            "gate must refuse the same shape"
        );
    }

    /// Gate and runtime now consume the SAME shape resolver, so for every
    /// row that RESOLVES they agree: a billable first row prices at runtime
    /// and passes the gate; a zero-priced first row fails the gate AND would
    /// produce a skipped hold at runtime. Shapeless rows (empty tiers) stay
    /// transparent in both paths (round-4: a shapeless row does not shadow).
    #[test]
    fn gate_and_runtime_agree_on_every_resolvable_row() {
        // Zero-priced row: runtime resolves to need_fen = 0 (hold skipped,
        // free-ride) → gate must refuse; runtime `price` still returns Some
        // (the debit layer's billable check owns that refusal).
        let zero_json = r#"[{"model_contains":"dup","input":0,"output":0}]"#;
        let rows: Vec<RateRow> = serde_json::from_str(zero_json).unwrap();
        let at = Utc.with_ymd_and_hms(2026, 8, 17, 2, 0, 0).unwrap();
        assert!(rows[0].rates(1000, at).is_some(), "zero row still resolves");
        assert!(!rates_present_in(zero_json, "any", "dup"));

        // Billable row: resolves AND passes the gate — one truth.
        let good = r#"[{"model_contains":"dup","input":20,"output":80}]"#;
        let rows: Vec<RateRow> = serde_json::from_str(good).unwrap();
        assert_eq!(
            resolve_in(&rows, "any", "dup", 1000, at)
                .map(|r| price_from_rates(&r, 1000, 1000, 0)),
            Some(1)
        );
        assert!(rates_present_in(good, "any", "dup"));
    }
}
