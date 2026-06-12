use typeshare::typeshare;
use serde::{Deserialize, Serialize};

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageResponse {
    #[typeshare(serialized_as = "number")]
    pub used_tokens:        i64,
    #[typeshare(serialized_as = "number")]
    pub limit_tokens:        i64,
    #[typeshare(serialized_as = "number")]
    pub used_documents:        i64,
    #[typeshare(serialized_as = "number")]
    pub limit_documents:        i64,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanRow {
    pub id: String,
    pub name: String,
    #[typeshare(serialized_as = "number")]
    pub price:        i64,
    pub features: Vec<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlansResponse {
    pub plans: Vec<PlanRow>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionResponse {
    pub plan_id: String,
    pub status: String,
    pub current_period_end: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingOverview {
    #[typeshare(serialized_as = "number")]
    pub active_subscriptions:        i64,
    #[typeshare(serialized_as = "number")]
    pub past_due_subscriptions:        i64,
    #[typeshare(serialized_as = "number")]
    pub unpaid_subscriptions:        i64,
    #[typeshare(serialized_as = "number")]
    pub canceled_subscriptions:        i64,
}
