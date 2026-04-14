use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageResponse {
    pub used_tokens: i64,
    pub limit_tokens: i64,
    pub used_documents: i64,
    pub limit_documents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanRow {
    pub id: String,
    pub name: String,
    pub price: i64,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlansResponse {
    pub plans: Vec<PlanRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionResponse {
    pub plan_id: String,
    pub status: String,
    pub current_period_end: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingOverview {
    pub active_subscriptions: i64,
    pub past_due_subscriptions: i64,
    pub unpaid_subscriptions: i64,
    pub canceled_subscriptions: i64,
}
