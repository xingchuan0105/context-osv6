use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillableFeature {
    Summary,
    Planner,
    Answer,
    Search,
    Chat,
    GraphExtraction,
}

impl BillableFeature {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Planner => "planner",
            Self::Answer => "answer",
            Self::Search => "search",
            Self::Chat => "chat",
            Self::GraphExtraction => "graph_extraction",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteringContext {
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub feature: BillableFeature,
    pub stage: String,
    pub session_id: Option<Uuid>,
    pub document_id: Option<Uuid>,
    pub request_id: Option<String>,
    pub trace_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageSource {
    Actual,
    Estimated,
}

impl UsageSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Actual => "actual",
            Self::Estimated => "estimated",
        }
    }
}

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
    pub blocked_5h: bool,
    pub blocked_7d: bool,
    pub used_5h: i64,
    pub limit_5h: i64,
    pub used_7d: i64,
    pub limit_7d: i64,
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
