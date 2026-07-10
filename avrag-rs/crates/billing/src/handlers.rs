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
use crate::types::{
    CreateUsageExportRequest, UsageExportAccepted, UsageExportStatusResponse,
    UsageForecastResponse, UsageHistoryResponse, UsageWindowResponse,
};
use crate::BillingProvider;
use app_core::UsageLimitStorePort;
use chrono::{DateTime, Utc};
use uuid::Uuid;

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

/// ADR 0006: create usage export (billable rows only).
pub async fn handle_create_usage_export(
    store: Arc<dyn UsageLimitStorePort>,
    owner_user_id: Uuid,
    user_id: Uuid,
    body: CreateUsageExportRequest,
) -> ApiResponse<UsageExportAccepted> {
    let from = match DateTime::parse_from_rfc3339(&body.from) {
        Ok(dt) => dt.with_timezone(&Utc),
        Err(_) => {
            return ApiResponse::err("invalid_export_from", "from must be RFC3339 datetime");
        }
    };
    let to = match DateTime::parse_from_rfc3339(&body.to) {
        Ok(dt) => dt.with_timezone(&Utc),
        Err(_) => {
            return ApiResponse::err("invalid_export_to", "to must be RFC3339 datetime");
        }
    };
    match store
        .create_usage_export_job(owner_user_id, user_id, from, to, &body.format)
        .await
    {
        Ok(id) => {
            let status = store
                .get_usage_export_job(user_id, id)
                .await
                .ok()
                .flatten()
                .map(|j| j.status)
                .unwrap_or_else(|| "pending".to_string());
            ApiResponse::ok(UsageExportAccepted {
                export_id: id.to_string(),
                status,
            })
        }
        Err(error) => ApiResponse::err(error.code(), error.message()),
    }
}

pub async fn handle_get_usage_export(
    store: Arc<dyn UsageLimitStorePort>,
    user_id: Uuid,
    export_id: Uuid,
) -> ApiResponse<UsageExportStatusResponse> {
    match store.get_usage_export_job(user_id, export_id).await {
        Ok(Some(job)) => ApiResponse::ok(UsageExportStatusResponse {
            export_id: job.id.to_string(),
            status: job.status,
            format: job.format,
            from: job.range_from.to_rfc3339(),
            to: job.range_to.to_rfc3339(),
            row_count: job.row_count,
            result: job.result_text,
            error_message: job.error_message,
            created_at: job.created_at.to_rfc3339(),
            completed_at: job.completed_at.map(|t| t.to_rfc3339()),
        }),
        Ok(None) => ApiResponse::err("export_not_found", "export job not found"),
        Err(error) => ApiResponse::err(error.code(), error.message()),
    }
}
