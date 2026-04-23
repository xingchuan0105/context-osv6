use std::collections::HashMap;

use super::*;

#[test]
fn compute_usage_units_default_rates() {
    let units = compute_usage_units("dashscope", "qwen3.5-flash", 1000, 500);
    // (1000/1000)*1.0 + (500/1000)*2.0 = 1.0 + 1.0 = 2.0 → ceil = 2
    assert_eq!(units, 2);

    let units_zero = compute_usage_units("any", "any", 0, 0);
    assert_eq!(units_zero, 0);

    let units_tiny = compute_usage_units("any", "any", 100, 0);
    // (100/1000)*1.0 = 0.1 → ceil = 1
    assert_eq!(units_tiny, 1);
}

#[test]
fn billable_feature_serialization() {
    assert_eq!(
        serde_json::to_string(&BillableFeature::Answer).unwrap(),
        r#""answer""#
    );
    assert_eq!(
        serde_json::to_string(&BillableFeature::Summary).unwrap(),
        r#""summary""#
    );
}

#[test]
fn usage_window_blocked_when_exceeded() {
    let window = UsageWindow {
        used_units: 150,
        limit_units: 100,
        remaining_units: 0,
        percent_used: 150.0,
        blocked: true,
        next_relief_at: Some("2026-04-01T00:00:00Z".to_string()),
        blocked_until: Some("2026-04-01T00:00:00Z".to_string()),
    };
    assert!(window.blocked);
    assert_eq!(window.remaining_units, 0);
}

#[test]
fn usage_limit_response_serialization_roundtrip() {
    let response = UsageLimitResponse {
        policy: UsageLimitPolicy {
            enabled: true,
            rolling_5h_limit_units: 50,
            rolling_7d_limit_units: 500,
        },
        windows: UsageWindows {
            rolling_5h: UsageWindow {
                used_units: 10,
                limit_units: 50,
                remaining_units: 40,
                percent_used: 20.0,
                blocked: false,
                next_relief_at: None,
                blocked_until: None,
            },
            rolling_7d: UsageWindow {
                used_units: 100,
                limit_units: 500,
                remaining_units: 400,
                percent_used: 20.0,
                blocked: false,
                next_relief_at: None,
                blocked_until: None,
            },
        },
        breakdown: HashMap::from([
            ("answer".to_string(), 60),
            ("planner".to_string(), 20),
            ("search".to_string(), 20),
        ]),
        scope: UsageScope::PlanDefault {
            plan_id: "free".to_string(),
        },
        has_estimated_usage: false,
    };

    let json = serde_json::to_string(&response).unwrap();
    let parsed: UsageLimitResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.policy.rolling_5h_limit_units, 50);
    assert_eq!(parsed.windows.rolling_5h.used_units, 10);
    assert!(!parsed.has_estimated_usage);
}
