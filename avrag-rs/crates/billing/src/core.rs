use anyhow::Result;
use app_core::BillingStorePort;
use chrono::{Datelike, TimeZone, Utc};
use common::UserId;
use std::collections::HashMap;
use std::sync::Arc;

use crate::types::{
    BillingConfig, BillingProvider, PLAN_FREE, PLAN_PLUS, PLAN_PRO, Subscription,
    UsageForecastResponse, UsageHistoryResponse, UsageWindowResponse, WebhookClaim,
};

pub(crate) fn build_plan_payloads(
    config: &BillingConfig,
    current_plan_id: &str,
    quotas: &HashMap<String, Vec<serde_json::Value>>,
) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "plan_id": PLAN_FREE,
            "name": "Free",
            "description": "Starter plan for smaller personal workspaces and trial usage.",
            "price_label": config.price_label_for_plan(PLAN_FREE),
            "price_label_cny": config.price_label_cny_for_plan(PLAN_FREE),
            "price_label_usd": config.price_label_usd_for_plan(PLAN_FREE),
            "interval": "month",
            "checkout_available": false,
            "current": current_plan_id == PLAN_FREE,
            "quotas": quotas.get(PLAN_FREE).cloned().unwrap_or_default(),
        }),
        serde_json::json!({
            "plan_id": PLAN_PLUS,
            "name": "Plus",
            "description": "Daily quotas for active document ingestion and chat workflows.",
            "price_label": config.price_label_for_plan(PLAN_PLUS),
            "price_label_cny": config.price_label_cny_for_plan(PLAN_PLUS),
            "price_label_usd": config.price_label_usd_for_plan(PLAN_PLUS),
            "interval": "month",
            "checkout_available": config.checkout_available(PLAN_PLUS),
            "current": current_plan_id == PLAN_PLUS,
            "quotas": quotas.get(PLAN_PLUS).cloned().unwrap_or_default(),
        }),
        serde_json::json!({
            "plan_id": PLAN_PRO,
            "name": "Pro",
            "description": "Unlimited quota posture for heavier workloads.",
            "price_label": config.price_label_for_plan(PLAN_PRO),
            "price_label_cny": config.price_label_cny_for_plan(PLAN_PRO),
            "price_label_usd": config.price_label_usd_for_plan(PLAN_PRO),
            "interval": "month",
            "checkout_available": config.checkout_available(PLAN_PRO),
            "current": current_plan_id == PLAN_PRO,
            "quotas": quotas.get(PLAN_PRO).cloned().unwrap_or_default(),
        }),
    ]
}

fn map_store_error(error: common::AppError) -> anyhow::Error {
    anyhow::anyhow!(error.to_string())
}

pub(crate) async fn get_current_subscription(
    store: Arc<dyn BillingStorePort>,
    user_id: UserId,
) -> Result<Subscription> {
    store
        .get_current_subscription(user_id)
        .await
        .map_err(map_store_error)
}

pub(crate) async fn load_plan_quotas(
    store: Arc<dyn BillingStorePort>,
) -> Result<HashMap<String, Vec<serde_json::Value>>> {
    store.load_plan_quotas().await.map_err(map_store_error)
}

pub(crate) async fn load_usage(
    store: Arc<dyn BillingStorePort>,
    user_id: UserId,
) -> Result<HashMap<String, i64>> {
    store.load_usage(user_id).await.map_err(map_store_error)
}

pub(crate) async fn current_metric_usage(
    store: Arc<dyn BillingStorePort>,
    user_id: UserId,
    metric_type: &str,
) -> Result<i64> {
    store
        .current_metric_usage(user_id, metric_type)
        .await
        .map_err(map_store_error)
}

pub(crate) async fn load_quota_limit(
    store: Arc<dyn BillingStorePort>,
    plan_id: &str,
    metric_type: &str,
) -> Result<Option<(Option<i64>, Option<i64>)>> {
    store
        .load_quota_limit(plan_id, metric_type)
        .await
        .map_err(map_store_error)
}

pub(crate) async fn load_usage_window(
    store: Arc<dyn BillingStorePort>,
    user_id: UserId,
) -> Result<UsageWindowResponse> {
    store
        .load_usage_window(user_id)
        .await
        .map_err(map_store_error)
}

pub(crate) async fn load_usage_history(
    store: Arc<dyn BillingStorePort>,
    user_id: UserId,
    days: i32,
) -> Result<UsageHistoryResponse> {
    store
        .load_usage_history(user_id, days)
        .await
        .map_err(map_store_error)
}

pub(crate) async fn load_usage_forecast(
    store: Arc<dyn BillingStorePort>,
    user_id: UserId,
) -> Result<UsageForecastResponse> {
    store
        .load_usage_forecast(user_id)
        .await
        .map_err(map_store_error)
}

pub(crate) async fn claim_webhook_with_lease(
    store: Arc<dyn BillingStorePort>,
    provider: BillingProvider,
    event_id: &str,
) -> Result<WebhookClaim> {
    store
        .claim_webhook_with_lease(provider, event_id)
        .await
        .map_err(map_store_error)
}

pub(crate) async fn update_webhook_lease_status(
    store: Arc<dyn BillingStorePort>,
    provider: BillingProvider,
    event_id: &str,
    status: &str,
    error: Option<String>,
) -> Result<()> {
    store
        .update_webhook_lease_status(provider, event_id, status, error)
        .await
        .map_err(map_store_error)
}

pub(crate) async fn process_webhook_event(
    store: Arc<dyn BillingStorePort>,
    provider: BillingProvider,
    payload: &serde_json::Value,
    config: &BillingConfig,
) -> Result<()> {
    store
        .process_webhook_event(provider, payload, config)
        .await
        .map_err(map_store_error)
}

pub(crate) fn seconds_until_next_month() -> u64 {
    let now = Utc::now();
    let (year, month) = if now.month() == 12 {
        (now.year() + 1, 1)
    } else {
        (now.year(), now.month() + 1)
    };
    let next = Utc
        .with_ymd_and_hms(year, month, 1, 0, 0, 0)
        .single()
        .expect("valid next month start");
    next.signed_duration_since(now).num_seconds().max(1) as u64
}

pub async fn expire_subscriptions(store: Arc<dyn BillingStorePort>) -> Result<()> {
    store.expire_subscriptions().await.map_err(map_store_error)
}

pub async fn process_outbox(store: Arc<dyn BillingStorePort>) -> Result<()> {
    store.process_outbox().await.map_err(map_store_error)
}
