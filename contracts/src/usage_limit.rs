use typeshare::typeshare;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageLimitResponse {
    pub policy: UsageLimitPolicy,
    pub windows: UsageWindows,
    #[typeshare(serialized_as = "Record<string, number>")]
    pub breakdown:        HashMap<String, i64>,
    pub scope: UsageScope,
    #[serde(default)]
    pub has_estimated_usage: bool,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsageLimitPolicy {
    pub enabled: bool,
    #[typeshare(serialized_as = "number")]
    pub rolling_5h_limit_units:        i64,
    #[typeshare(serialized_as = "number")]
    pub rolling_7d_limit_units:        i64,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageWindows {
    pub rolling_5h: UsageWindow,
    pub rolling_7d: UsageWindow,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageWindow {
    #[typeshare(serialized_as = "number")]
    pub used_units:        i64,
    #[typeshare(serialized_as = "number")]
    pub limit_units:        i64,
    #[typeshare(serialized_as = "number")]
    pub remaining_units:        i64,
    pub percent_used: f64,
    pub blocked: bool,
    pub next_relief_at: Option<String>,
    pub blocked_until: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "content")]
pub enum UsageScope {
    #[serde(rename = "plan_default")]
    PlanDefault { plan_id: String },
    #[serde(rename = "user_override")]
    UserOverride,
}
