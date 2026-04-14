/// Compute usage_units from token counts using model weight table.
/// Falls back to a default formula if no weight row exists.
pub fn compute_usage_units(
    _provider: &str,
    _model: &str,
    prompt_tokens: u32,
    completion_tokens: u32,
) -> i64 {
    compute_usage_units_with_rates(prompt_tokens, completion_tokens, 1.0, 2.0)
}

/// Compute usage_units from explicit input/output rates.
pub fn compute_usage_units_with_rates(
    prompt_tokens: u32,
    completion_tokens: u32,
    input_rate: f64,
    output_rate: f64,
) -> i64 {
    // Default: 1 unit per 1K input tokens, 2 units per 1K output tokens
    if prompt_tokens == 0 && completion_tokens == 0 {
        return 0;
    }

    let input_units = (prompt_tokens as f64 / 1000.0) * input_rate;
    let output_units = (completion_tokens as f64 / 1000.0) * output_rate;

    (input_units + output_units).ceil().max(1.0) as i64
}
