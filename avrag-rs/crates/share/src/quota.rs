//! ADR-0010: plan → max share-enabled workspaces (product defaults).
//!
//! Runtime PG reads `usage_limit_plan_policies.max_shared_workspaces`; this
//! module is the canonical fallback / unit-test policy surface.

/// Stable AppError code when enabling a share would exceed the plan quota.
pub const SHARE_WORKSPACE_QUOTA_EXCEEDED: &str = "share_workspace_quota_exceeded";

/// Max workspaces with `share_enabled = true` for a raw `plan_id`.
///
/// free=3, plus=10, pro=100. Legacy aliases normalize like billing tiers.
pub fn max_shared_workspaces_for_plan(plan_id: &str) -> i32 {
    match plan_id.trim().to_lowercase().as_str() {
        "plus" | "starter" | "team" | "enterprise" => 10,
        "pro" => 100,
        // free, empty, unknown → free defaults
        _ => 3,
    }
}

/// Whether transitioning workspace access_level occupies a share slot.
pub fn access_level_enables_share(access_level: &str) -> bool {
    matches!(access_level.trim(), "link" | "public")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_plus_pro_limits() {
        assert_eq!(max_shared_workspaces_for_plan("free"), 3);
        assert_eq!(max_shared_workspaces_for_plan("plus"), 10);
        assert_eq!(max_shared_workspaces_for_plan("pro"), 100);
    }

    #[test]
    fn legacy_aliases_map_to_plus() {
        assert_eq!(max_shared_workspaces_for_plan("enterprise"), 10);
        assert_eq!(max_shared_workspaces_for_plan("starter"), 10);
        assert_eq!(max_shared_workspaces_for_plan("team"), 10);
    }

    #[test]
    fn unknown_plan_defaults_to_free() {
        assert_eq!(max_shared_workspaces_for_plan(""), 3);
        assert_eq!(max_shared_workspaces_for_plan("e2e"), 3);
        assert_eq!(max_shared_workspaces_for_plan("mystery"), 3);
    }

    #[test]
    fn access_level_gate() {
        assert!(!access_level_enables_share("private"));
        assert!(access_level_enables_share("link"));
        assert!(access_level_enables_share("public"));
    }
}
