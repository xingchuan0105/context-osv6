use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageLimitResponse {
    pub policy: UsageLimitPolicy,
    pub windows: UsageWindows,
    pub breakdown: HashMap<String, i64>,
    pub scope: UsageScope,
    #[serde(default)]
    pub has_estimated_usage: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsageLimitPolicy {
    pub enabled: bool,
    pub rolling_5h_limit_units: i64,
    pub rolling_7d_limit_units: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageWindows {
    pub rolling_5h: UsageWindow,
    pub rolling_7d: UsageWindow,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageWindow {
    pub used_units: i64,
    pub limit_units: i64,
    pub remaining_units: i64,
    pub percent_used: f64,
    pub blocked: bool,
    pub next_relief_at: Option<String>,
    pub blocked_until: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum UsageScope {
    #[serde(rename = "plan_default")]
    PlanDefault { plan_id: String },
    #[serde(rename = "user_override")]
    UserOverride,
}
