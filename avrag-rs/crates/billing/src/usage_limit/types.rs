use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use app_core::{BillableFeature, MeteringContext, UsageSource};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageLimitPolicy {
    pub enabled: bool,
    pub rolling_5h_limit_units: i64,
    pub rolling_7d_limit_units: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageWindow {
    pub used_units: i64,
    pub limit_units: i64,
    pub remaining_units: i64,
    pub percent_used: f64,
    pub blocked: bool,
    pub next_relief_at: Option<String>,
    pub blocked_until: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageWindows {
    pub rolling_5h: UsageWindow,
    pub rolling_7d: UsageWindow,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QuotaCheckResult {
    /// Soft limit exceeded (used ≥ plan limit). Requests may still proceed.
    pub soft_exceeded_5h: bool,
    pub soft_exceeded_7d: bool,
    /// Abuse hard-cap exceeded (used ≥ limit × hard_cap_multiplier). Must hard-block.
    pub blocked_5h: bool,
    pub blocked_7d: bool,
    pub used_5h: i64,
    pub limit_5h: i64,
    pub used_7d: i64,
    pub limit_7d: i64,
    pub hard_cap_5h: i64,
    pub hard_cap_7d: i64,
    pub blocked_until_5h: Option<DateTime<Utc>>,
    pub blocked_until_7d: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UsageScope {
    #[serde(rename = "plan_default")]
    PlanDefault { plan_id: String },
    #[serde(rename = "user_override")]
    UserOverride,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageLimitResponse {
    pub policy: UsageLimitPolicy,
    pub windows: UsageWindows,
    pub breakdown: HashMap<String, i64>,
    pub scope: UsageScope,
    pub has_estimated_usage: bool,
}
