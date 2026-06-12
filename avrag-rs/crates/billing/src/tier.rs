//! Canonical billing tier vocabulary and agent loop budget policy.
//!
//! Product tiers are `Free | Plus | Pro`. Legacy `enterprise` (and other aliases)
//! normalize to [`BillingTier::Plus`] for application logic; the `enterprise`
//! row in `usage_limit_plan_policies` remains a DB-only migration alias.

use serde::{Deserialize, Serialize};

use crate::types::{PLAN_FREE, PLAN_PLUS, PLAN_PRO};

/// Canonical subscription tier — the ubiquitous language for billing and quotas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BillingTier {
    Free,
    Plus,
    Pro,
}

/// Agent mode for ReAct loop iteration budgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactLoopAgentMode {
    Rag,
    Search,
    Chat,
}

/// Tier-aware ReAct loop iteration ceilings (single source of truth).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReactLoopBudgetPolicy;

impl BillingTier {
    /// Normalize a raw `plan_id` (including legacy DB aliases) to a canonical tier.
    pub fn from_plan_id(plan_id: &str) -> Self {
        match plan_id.trim().to_lowercase().as_str() {
            PLAN_FREE | "" => Self::Free,
            PLAN_PLUS | "starter" | "team" | "enterprise" => Self::Plus,
            PLAN_PRO => Self::Pro,
            _ => Self::Free,
        }
    }

    /// Canonical `plan_id` string for API payloads and display.
    pub fn plan_id(self) -> &'static str {
        match self {
            Self::Free => PLAN_FREE,
            Self::Plus => PLAN_PLUS,
            Self::Pro => PLAN_PRO,
        }
    }

    /// Human-readable tier name (English).
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Plus => "Plus",
            Self::Pro => "Pro",
        }
    }
}

impl ReactLoopBudgetPolicy {
    /// Max ReAct iterations for the given agent mode and tier.
    ///
    /// Plus inherits the former Enterprise/Pro mid-tier ceilings; Pro is the top tier.
    pub fn max_iterations(mode: ReactLoopAgentMode, tier: BillingTier) -> u8 {
        match (mode, tier) {
            (ReactLoopAgentMode::Rag, BillingTier::Free) => 2,
            (ReactLoopAgentMode::Rag, BillingTier::Plus | BillingTier::Pro) => 4,
            (ReactLoopAgentMode::Search, BillingTier::Free) => 2,
            (ReactLoopAgentMode::Search, BillingTier::Plus | BillingTier::Pro) => 3,
            (ReactLoopAgentMode::Chat, BillingTier::Free) => 2,
            (ReactLoopAgentMode::Chat, BillingTier::Plus | BillingTier::Pro) => 3,
        }
    }

    pub fn rag(tier: BillingTier) -> u8 {
        Self::max_iterations(ReactLoopAgentMode::Rag, tier)
    }

    pub fn search(tier: BillingTier) -> u8 {
        Self::max_iterations(ReactLoopAgentMode::Search, tier)
    }

    pub fn chat(tier: BillingTier) -> u8 {
        Self::max_iterations(ReactLoopAgentMode::Chat, tier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enterprise_normalizes_to_plus() {
        assert_eq!(BillingTier::from_plan_id("enterprise"), BillingTier::Plus);
        assert_eq!(BillingTier::from_plan_id("ENTERPRISE"), BillingTier::Plus);
    }

    #[test]
    fn canonical_plan_ids_roundtrip() {
        for (raw, tier) in [
            ("free", BillingTier::Free),
            ("plus", BillingTier::Plus),
            ("pro", BillingTier::Pro),
            ("enterprise", BillingTier::Plus),
        ] {
            assert_eq!(BillingTier::from_plan_id(raw), tier);
            assert_eq!(
                tier.plan_id(),
                if raw == "enterprise" { "plus" } else { raw }
            );
        }
    }

    #[test]
    fn react_loop_budget_matches_product_tiers() {
        assert_eq!(ReactLoopBudgetPolicy::rag(BillingTier::Free), 2);
        assert_eq!(ReactLoopBudgetPolicy::rag(BillingTier::Plus), 4);
        assert_eq!(ReactLoopBudgetPolicy::rag(BillingTier::Pro), 4);
        assert_eq!(ReactLoopBudgetPolicy::search(BillingTier::Free), 2);
        assert_eq!(ReactLoopBudgetPolicy::search(BillingTier::Plus), 3);
        assert_eq!(ReactLoopBudgetPolicy::chat(BillingTier::Pro), 3);
    }
}
