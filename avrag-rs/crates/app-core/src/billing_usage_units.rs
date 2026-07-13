/// Compute usage_units from token counts using default Flash-relative rates
/// (miss=1.0, cache_hit=0.02, out=2.0) and margin multiplier M=1.0.
pub fn compute_usage_units(
    _provider: &str,
    _model: &str,
    prompt_tokens: u32,
    completion_tokens: u32,
) -> i64 {
    compute_usage_units_with_rates(prompt_tokens, completion_tokens, 1.0, 2.0)
}

/// Two-bucket helper (cached=0). Prefer [`compute_usage_units_three_bucket`] for product metering.
pub fn compute_usage_units_with_rates(
    prompt_tokens: u32,
    completion_tokens: u32,
    input_rate: f64,
    output_rate: f64,
) -> i64 {
    compute_usage_units_three_bucket(
        prompt_tokens,
        completion_tokens,
        0,
        input_rate,
        0.02,
        output_rate,
        1.0,
    )
}

/// DeepSeek-style three-bucket units with plan margin multiplier M.
///
/// ```text
/// raw = miss/1k * rate_miss + cached/1k * rate_cache + out/1k * rate_out
/// units = 0 if no tokens else max(1, ceil(raw * M))
/// ```
pub fn compute_usage_units_three_bucket(
    prompt_tokens: u32,
    completion_tokens: u32,
    cached_tokens: u32,
    rate_miss: f64,
    rate_cache: f64,
    rate_out: f64,
    margin_multiplier: f64,
) -> i64 {
    if prompt_tokens == 0 && completion_tokens == 0 {
        return 0;
    }

    let cached = cached_tokens.min(prompt_tokens);
    let miss = prompt_tokens.saturating_sub(cached);
    let raw = (miss as f64 / 1000.0) * rate_miss
        + (cached as f64 / 1000.0) * rate_cache
        + (completion_tokens as f64 / 1000.0) * rate_out;
    let m = if margin_multiplier.is_finite() && margin_multiplier > 0.0 {
        margin_multiplier
    } else {
        1.0
    };
    (raw * m).ceil().max(1.0) as i64
}

/// Approximate product "约 tokens" from usage_units given plan M.
/// Under pure cache-miss input: tokens ≈ units / M * 1000.
pub fn tokens_approx_from_units(units: i64, margin_multiplier: f64) -> i64 {
    let m = if margin_multiplier.is_finite() && margin_multiplier > 0.0 {
        margin_multiplier
    } else {
        1.0
    };
    ((units as f64) / m * 1000.0).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_bucket_wrapper_matches_three_bucket_cached_zero() {
        let a = compute_usage_units_with_rates(1000, 0, 1.0, 2.0);
        let b = compute_usage_units_three_bucket(1000, 0, 0, 1.0, 0.02, 2.0, 1.0);
        assert_eq!(a, b);
        assert_eq!(a, 1);
    }

    #[test]
    fn pure_miss_with_margin_ceil() {
        // raw = 1.0, M=1.5 → ceil(1.5) = 2
        assert_eq!(
            compute_usage_units_three_bucket(1000, 0, 0, 1.0, 0.02, 2.0, 1.5),
            2
        );
    }

    #[test]
    fn full_cache_hit_still_at_least_one() {
        // raw = 0.02, M=1 → ceil → 1
        assert_eq!(
            compute_usage_units_three_bucket(1000, 0, 1000, 1.0, 0.02, 2.0, 1.0),
            1
        );
    }

    #[test]
    fn design_example_plus_m15() {
        // prompt 20k (hit 16k), out 2k → raw 8.32 → *1.5 = 12.48 → 13
        assert_eq!(
            compute_usage_units_three_bucket(20_000, 2_000, 16_000, 1.0, 0.02, 2.0, 1.5),
            13
        );
    }

    #[test]
    fn free_m_higher_than_pro_same_raw() {
        let free = compute_usage_units_three_bucket(5000, 1000, 0, 1.0, 0.02, 2.0, 2.0);
        let pro = compute_usage_units_three_bucket(5000, 1000, 0, 1.0, 0.02, 2.0, 1.3);
        assert!(free > pro);
    }

    #[test]
    fn zero_tokens_zero_units() {
        assert_eq!(
            compute_usage_units_three_bucket(0, 0, 0, 1.0, 0.02, 2.0, 2.0),
            0
        );
    }

    #[test]
    fn clamp_cached_above_prompt() {
        // cached clamped to prompt → same as full hit
        assert_eq!(
            compute_usage_units_three_bucket(1000, 0, 5000, 1.0, 0.02, 2.0, 1.0),
            compute_usage_units_three_bucket(1000, 0, 1000, 1.0, 0.02, 2.0, 1.0)
        );
    }

    #[test]
    fn tokens_approx_roundtrip_free_limit() {
        // free 5h: 200 units, M=2 → 100_000 tokens
        assert_eq!(tokens_approx_from_units(200, 2.0), 100_000);
        assert_eq!(tokens_approx_from_units(900, 1.5), 600_000);
        assert_eq!(tokens_approx_from_units(3250, 1.3), 2_500_000);
    }
}
