use anyhow::Result;
use app_core::BillingStorePort;
use common::{ApiResponse, UserId};
use std::sync::Arc;

use crate::core::{
    get_current_subscription, load_usage, load_usage_forecast, load_usage_history,
    load_usage_window,
};
use crate::service::{
    BillingService, CheckoutResponse, CreateCheckoutRequest, PortalResponse, QuotaDecision,
    SubscriptionResponse, UsageResponse,
};
use crate::types::{UsageForecastResponse, UsageHistoryResponse, UsageWindowResponse};
use crate::BillingProvider;

pub async fn handle_get_plans(
    store: Arc<dyn BillingStorePort>,
    user_id: UserId,
) -> ApiResponse<serde_json::Value> {
    BillingService::shared().get_plans(store, user_id).await
}

pub async fn handle_get_subscription(
    store: Arc<dyn BillingStorePort>,
    user_id: UserId,
) -> ApiResponse<SubscriptionResponse> {
    match get_current_subscription(store, user_id).await {
        Ok(subscription) => ApiResponse::ok(SubscriptionResponse { subscription }),
        Err(error) => ApiResponse::err("billing_subscription_failed", &error.to_string()),
    }
}

pub async fn handle_get_usage(
    store: Arc<dyn BillingStorePort>,
    user_id: UserId,
) -> ApiResponse<UsageResponse> {
    match load_usage(store, user_id).await {
        Ok(usage) => ApiResponse::ok(UsageResponse { usage }),
        Err(error) => ApiResponse::err("billing_usage_failed", &error.to_string()),
    }
}

pub async fn handle_get_usage_window(
    store: Arc<dyn BillingStorePort>,
    user_id: UserId,
) -> ApiResponse<UsageWindowResponse> {
    match load_usage_window(store, user_id).await {
        Ok(window) => ApiResponse::ok(window),
        Err(error) => ApiResponse::err("billing_usage_window_failed", &error.to_string()),
    }
}

pub async fn handle_get_usage_history(
    store: Arc<dyn BillingStorePort>,
    user_id: UserId,
    days: i32,
) -> ApiResponse<UsageHistoryResponse> {
    match load_usage_history(store, user_id, days).await {
        Ok(history) => ApiResponse::ok(history),
        Err(error) => ApiResponse::err("billing_usage_history_failed", &error.to_string()),
    }
}

pub async fn handle_get_usage_forecast(
    store: Arc<dyn BillingStorePort>,
    user_id: UserId,
) -> ApiResponse<UsageForecastResponse> {
    match load_usage_forecast(store, user_id).await {
        Ok(forecast) => ApiResponse::ok(forecast),
        Err(error) => ApiResponse::err("billing_usage_forecast_failed", &error.to_string()),
    }
}

pub async fn handle_create_checkout(
    store: Arc<dyn BillingStorePort>,
    user_id: UserId,
    body: CreateCheckoutRequest,
) -> ApiResponse<CheckoutResponse> {
    BillingService::shared()
        .create_checkout(store, user_id, body)
        .await
}

pub async fn handle_create_portal(
    _store: Arc<dyn BillingStorePort>,
    _user_id: UserId,
) -> ApiResponse<PortalResponse> {
    ApiResponse::err(
        "billing_portal_unavailable",
        "Self-service billing portal is unavailable; manage subscriptions via Creem or contact support",
    )
}

pub async fn handle_webhook(
    store: Arc<dyn BillingStorePort>,
    provider: BillingProvider,
    signature: Option<&str>,
    payload: &[u8],
) -> ApiResponse<serde_json::Value> {
    BillingService::shared()
        .handle_webhook(store, provider, signature, payload)
        .await
}

pub async fn check_quota(
    store: Arc<dyn BillingStorePort>,
    user_id: UserId,
    metric_type: &str,
    requested: i64,
) -> Result<QuotaDecision> {
    BillingService::shared()
        .check_quota(store, user_id, metric_type, requested)
        .await
}
