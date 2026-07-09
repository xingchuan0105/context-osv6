use async_trait::async_trait;
use chrono::{DateTime, Utc};
use common::{AppError, UserId};
use std::collections::HashMap;
use uuid::Uuid;

use crate::billing_domain::{
    BillingConfig, BillingProvider, MeteringContext, Subscription, UsageForecastResponse,
    UsageHistoryResponse, UsageSource, UsageWindowResponse, WebhookClaim,
};

#[derive(Debug, Clone)]
pub struct UsageLimitOverrideRow {
    pub rolling_5h_limit_units: Option<i64>,
    pub rolling_7d_limit_units: Option<i64>,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct UsageLimitPlanPolicyRow {
    pub enabled: bool,
    pub rolling_5h_limit_units: i64,
    pub rolling_7d_limit_units: i64,
}

pub struct UsageLimitUsageRecord<'a> {
    pub provider: &'a str,
    pub model: &'a str,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub usage_source: UsageSource,
    /// Exit-metering kind: `chat`, `embedding_text`, `embedding_multimodal`, …
    pub usage_kind: &'a str,
}

/// Billing persistence boundary — SQL implementations live in bootstrap adapters.
#[async_trait]
pub trait BillingStorePort: Send + Sync {
    async fn get_current_subscription(&self, user_id: UserId) -> Result<Subscription, AppError>;

    async fn load_plan_quotas(&self) -> Result<HashMap<String, Vec<serde_json::Value>>, AppError>;

    async fn load_usage(&self, user_id: UserId) -> Result<HashMap<String, i64>, AppError>;

    async fn current_metric_usage(
        &self,
        user_id: UserId,
        metric_type: &str,
    ) -> Result<i64, AppError>;

    async fn load_quota_limit(
        &self,
        plan_id: &str,
        metric_type: &str,
    ) -> Result<Option<(Option<i64>, Option<i64>)>, AppError>;

    async fn load_customer_id(&self, user_id: UserId) -> Result<Option<String>, AppError>;

    async fn load_user_contact(&self, user_id: UserId) -> Result<(String, String), AppError>;

    async fn save_stripe_customer_id(
        &self,
        user_id: UserId,
        customer_id: &str,
    ) -> Result<(), AppError>;

    async fn load_usage_window(&self, user_id: UserId) -> Result<UsageWindowResponse, AppError>;

    async fn load_usage_history(
        &self,
        user_id: UserId,
        days: i32,
    ) -> Result<UsageHistoryResponse, AppError>;

    async fn load_usage_forecast(&self, user_id: UserId)
    -> Result<UsageForecastResponse, AppError>;

    async fn insert_pending_alipay_order(
        &self,
        user_id: UserId,
        out_trade_no: &str,
        plan_id: &str,
        amount_cents: i64,
    ) -> Result<(), AppError>;

    async fn claim_webhook_with_lease(
        &self,
        provider: BillingProvider,
        event_id: &str,
    ) -> Result<WebhookClaim, AppError>;

    async fn update_webhook_lease_status(
        &self,
        provider: BillingProvider,
        event_id: &str,
        status: &str,
        error: Option<String>,
    ) -> Result<(), AppError>;

    async fn process_webhook_event(
        &self,
        provider: BillingProvider,
        payload: &serde_json::Value,
        config: &BillingConfig,
    ) -> Result<(), AppError>;

    async fn expire_subscriptions(&self) -> Result<(), AppError>;

    async fn process_outbox(&self) -> Result<(), AppError>;
}

/// Rolling usage-limit persistence boundary — SQL implementations live in bootstrap adapters.
#[async_trait]
pub trait UsageLimitStorePort: Send + Sync {
    async fn insert_llm_usage_event(
        &self,
        ctx: &MeteringContext,
        record: UsageLimitUsageRecord<'_>,
    ) -> Result<i64, AppError>;

    async fn load_user_override(
        &self,
        user_id: Uuid,
    ) -> Result<Option<UsageLimitOverrideRow>, AppError>;

    async fn get_user_plan(&self, user_id: Uuid) -> Result<String, AppError>;

    async fn load_plan_policy(
        &self,
        plan_id: &str,
    ) -> Result<Option<UsageLimitPlanPolicyRow>, AppError>;

    async fn sum_usage_units_since(
        &self,
        user_id: Uuid,
        since: DateTime<Utc>,
    ) -> Result<i64, AppError>;

    async fn oldest_usage_event_since(
        &self,
        user_id: Uuid,
        since: DateTime<Utc>,
    ) -> Result<Option<DateTime<Utc>>, AppError>;

    async fn load_usage_breakdown(
        &self,
        user_id: Uuid,
        since: DateTime<Utc>,
    ) -> Result<HashMap<String, i64>, AppError>;

    async fn load_model_rates(&self, provider: &str, model: &str) -> Result<(f64, f64), AppError>;

    async fn has_user_override(&self, user_id: Uuid) -> Result<bool, AppError>;

    async fn has_estimated_usage(&self, user_id: Uuid) -> Result<bool, AppError>;
}
