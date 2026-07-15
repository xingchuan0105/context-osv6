//! Product write E2E — parked: write product offline (2026-07-15).
//!
//! Historical multi-phase HeavyTail write path is rejected at `ConversationApp`
//! with `write_mode_disabled`. Do not re-enable success assertions until write
//! is product-online again.

/// Parked real-LLM write article pipeline.
///
/// Product boundary returns `write_mode_disabled` for `agent_type=write`.
/// Re-enable only after write product lane is restored.
#[tokio::test]
#[ignore = "write product offline 2026-07-15"]
async fn real_llm_write_mode_produces_article_with_fingerprint() {
    // Intentionally empty: product write is offline. See ConversationApp +
    // app_chat::write_disabled_error (`write_mode_disabled`).
}
