//! Pure Write-mode product contracts (mock-testable without LLM).

use common::AppError;

/// Canonical chat `agent_type` / mode string for Write.
pub const WRITE_AGENT_TYPE: &str = "write";
pub const WRITE_MODE: &str = "write";

/// Reject empty / whitespace-only write topics (orchestrator invariant).
pub fn require_non_empty_write_topic(topic: &str) -> Result<(), AppError> {
    if topic.trim().is_empty() {
        return Err(AppError::validation(
            "empty_write_topic",
            "Write mode requires a non-empty topic",
        ));
    }
    Ok(())
}

/// ADR 0006 §2: Write usage lands in the same user rolling ledger; product
/// billing must **not** invent a separate Write SKU / bill line.
pub fn write_usage_is_unified_billing() -> bool {
    true
}

/// Internal LLM feature tags for Write phases (`write:refine`, `write:draft`, …).
/// Used for cost analysis only; must still map into the unified user ledger
/// (see `app-billing` `map_feature`), never a separate product bill line.
pub fn is_write_internal_feature_tag(feature: &str) -> bool {
    let f = feature.trim();
    f.starts_with("write:") || f.starts_with("write_")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// S4 P-Terminal (CAP-WRITE): empty topic must never enter write pipeline.
    #[test]
    fn patho_terminal_empty_write_topic_rejected() {
        let err = require_non_empty_write_topic("").unwrap_err();
        assert_eq!(err.code(), "empty_write_topic");
        let err = require_non_empty_write_topic("   \n").unwrap_err();
        assert_eq!(err.code(), "empty_write_topic");
    }

    #[test]
    fn non_empty_topic_ok() {
        assert!(require_non_empty_write_topic("写一篇关于 Rust 的短文").is_ok());
    }

    #[test]
    fn agent_type_and_mode_are_write() {
        assert_eq!(WRITE_AGENT_TYPE, "write");
        assert_eq!(WRITE_MODE, "write");
    }

    #[test]
    fn billing_does_not_split_write_sku() {
        assert!(write_usage_is_unified_billing());
    }

    #[test]
    fn write_phase_tags_are_internal_not_sku() {
        assert!(is_write_internal_feature_tag("write:refine"));
        assert!(is_write_internal_feature_tag("write:draft"));
        assert!(is_write_internal_feature_tag("write_research"));
        assert!(!is_write_internal_feature_tag("agent_loop"));
        assert!(!is_write_internal_feature_tag("chat"));
        // High usage does not change the product rule: still unified billing.
        assert!(write_usage_is_unified_billing());
    }
}
