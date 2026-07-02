pub fn env_flag_enabled(key: &str, default: bool) -> bool {
    std::env::var(key)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

pub fn vlm_summary_enabled() -> bool {
    env_flag_enabled("INGESTION_VLM_SUMMARY_ENABLED", true)
}

pub fn triplet_batch_token_budget() -> i64 {
    std::env::var("INGESTION_TRIPLET_TOKEN_BUDGET")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(3_000)
}
