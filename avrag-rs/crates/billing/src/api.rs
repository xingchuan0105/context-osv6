use anyhow::Result;
use avrag_storage_pg::PgAppRepository;
use common::{ApiResponse, OrgId, UserId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::core::{
    build_plan_payloads, claim_webhook, current_metric_usage, ensure_customer,
    get_current_subscription, load_customer_id, load_plan_quotas, load_quota_limit, load_usage,
    process_webhook_event, seconds_until_next_month, update_webhook_status,
};
use crate::types::{PLAN_FREE, PLAN_PRO};
use crate::{BillingConfig, StripeClient, Subscription};

#[derive(Deserialize)]
pub struct CreateCheckoutRequest {
    pub plan_id: Option<String>,
}

#[derive(Serialize)]
pub struct CheckoutResponse {
    pub url: String,
    pub session_id: String,
}

#[derive(Serialize)]
pub struct PortalResponse {
    pub url: String,
}

#[derive(Serialize)]
pub struct SubscriptionResponse {
    pub subscription: Subscription,
}

#[derive(Serialize)]
pub struct UsageResponse {
    pub usage: HashMap<String, i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaDecision {
    pub plan_id: String,
    pub metric_type: String,
    pub current_usage: i64,
    pub soft_limit: Option<i64>,
    pub hard_limit: Option<i64>,
    pub requested: i64,
    pub allowed: bool,
    pub retry_after_secs: u64,
}

pub async fn handle_get_plans(
    repo: Arc<PgAppRepository>,
    org_id: OrgId,
) -> ApiResponse<serde_json::Value> {
    let config = BillingConfig::from_env();
    let subscription = match get_current_subscription(repo.clone(), org_id).await {
        Ok(sub) => sub,
        Err(error) => return ApiResponse::err("billing_plans_failed", &error.to_string()),
    };
    let current_plan_id = subscription.plan_id.clone();
    let quotas = match load_plan_quotas(repo).await {
        Ok(quotas) => quotas,
        Err(error) => return ApiResponse::err("billing_plans_failed", &error.to_string()),
    };

    ApiResponse::ok(serde_json::json!({
        "plans": build_plan_payloads(&config, &current_plan_id, &quotas),
        "current_plan_id": current_plan_id,
    }))
}

pub async fn handle_get_subscription(
    repo: Arc<PgAppRepository>,
    org_id: OrgId,
) -> ApiResponse<SubscriptionResponse> {
    match get_current_subscription(repo, org_id).await {
        Ok(subscription) => ApiResponse::ok(SubscriptionResponse { subscription }),
        Err(error) => ApiResponse::err("billing_subscription_failed", &error.to_string()),
    }
}

pub async fn handle_get_usage(
    repo: Arc<PgAppRepository>,
    org_id: OrgId,
) -> ApiResponse<UsageResponse> {
    match load_usage(repo, org_id).await {
        Ok(usage) => ApiResponse::ok(UsageResponse { usage }),
        Err(error) => ApiResponse::err("billing_usage_failed", &error.to_string()),
    }
}

pub async fn handle_create_checkout(
    repo: Arc<PgAppRepository>,
    org_id: OrgId,
    user_id: UserId,
    body: CreateCheckoutRequest,
) -> ApiResponse<CheckoutResponse> {
    let config = BillingConfig::from_env();
    let client = StripeClient::new(config.clone());
    if !config.stripe_enabled() {
        return ApiResponse::err("billing_unconfigured", "billing checkout is not configured");
    }

    let requested_plan = body.plan_id.as_deref().unwrap_or(PLAN_PRO).trim();
    if requested_plan == PLAN_FREE {
        return ApiResponse::err(
            "billing_plan_not_checkoutable",
            "free plan does not require checkout",
        );
    }
    let Some(price_id) = config
        .checkout_price_for_plan(requested_plan)
        .map(str::to_string)
    else {
        return ApiResponse::err(
            "invalid_billing_plan",
            "requested billing plan is not configured for checkout",
        );
    };

    match ensure_customer(repo.clone(), &client, org_id, user_id).await {
        Ok(customer_id) => {
            match client
                .create_checkout_session(&customer_id, &price_id, org_id, requested_plan)
                .await
            {
                Ok((url, session_id)) => ApiResponse::ok(CheckoutResponse { url, session_id }),
                Err(error) => ApiResponse::err("billing_checkout_failed", &error.to_string()),
            }
        }
        Err(error) => ApiResponse::err("billing_customer_failed", &error.to_string()),
    }
}

pub async fn handle_create_portal(
    repo: Arc<PgAppRepository>,
    org_id: OrgId,
) -> ApiResponse<PortalResponse> {
    let config = BillingConfig::from_env();
    let client = StripeClient::new(config.clone());
    if !config.stripe_enabled() {
        return ApiResponse::err("billing_unconfigured", "billing portal is not configured");
    }
    match load_customer_id(repo, org_id).await {
        Ok(Some(customer_id)) => match client.create_portal_session(&customer_id).await {
            Ok(url) => ApiResponse::ok(PortalResponse { url }),
            Err(error) => ApiResponse::err("billing_portal_failed", &error.to_string()),
        },
        Ok(None) => ApiResponse::err(
            "billing_portal_unavailable",
            "billing portal is unavailable before an active Stripe customer exists",
        ),
        Err(error) => ApiResponse::err("billing_customer_failed", &error.to_string()),
    }
}

pub async fn handle_webhook(
    repo: Arc<PgAppRepository>,
    signature: &str,
    payload: &[u8],
) -> ApiResponse<serde_json::Value> {
    let config = BillingConfig::from_env();
    let client = StripeClient::new(config.clone());
    if !config.webhook_enabled() {
        return ApiResponse::err("billing_unconfigured", "billing webhook is not configured");
    }
    if let Err(error) = client.verify_webhook_signature(payload, signature) {
        return ApiResponse::err("billing_webhook_signature_failed", &error.to_string());
    }
    let json: serde_json::Value = match serde_json::from_slice(payload) {
        Ok(value) => value,
        Err(error) => return ApiResponse::err("billing_webhook_invalid", &error.to_string()),
    };

    let claim = match claim_webhook(repo.clone(), &json).await {
        Ok(claim) => claim,
        Err(error) => return ApiResponse::err("billing_webhook_failed", &error.to_string()),
    };
    if claim.duplicate_processed {
        return ApiResponse::ok(serde_json::json!({
            "status": "ok",
            "duplicate": true,
        }));
    }

    if let Err(error) = process_webhook_event(repo.clone(), &json, &config).await {
        let _ =
            update_webhook_status(repo, &claim.event_id, "failed", Some(error.to_string())).await;
        return ApiResponse::err("billing_webhook_failed", &error.to_string());
    }

    if let Err(error) = update_webhook_status(repo, &claim.event_id, "processed", None).await {
        return ApiResponse::err("billing_webhook_failed", &error.to_string());
    }
    ApiResponse::ok(serde_json::json!({ "status": "ok" }))
}

pub async fn check_quota(
    repo: Arc<PgAppRepository>,
    org_id: OrgId,
    metric_type: &str,
    requested: i64,
) -> Result<QuotaDecision> {
    let subscription = get_current_subscription(repo.clone(), org_id).await?;
    let plan_id = subscription.plan_id;
    let quota = load_quota_limit(repo.clone(), &plan_id, metric_type).await?;
    let current_usage = current_metric_usage(repo, org_id, metric_type).await?;
    let hard_limit = quota.as_ref().and_then(|value| value.1);
    let soft_limit = quota.as_ref().and_then(|value| value.0);
    let allowed = hard_limit
        .map(|limit| current_usage.saturating_add(requested) <= limit)
        .unwrap_or(true);
    Ok(QuotaDecision {
        plan_id,
        metric_type: metric_type.to_string(),
        current_usage,
        soft_limit,
        hard_limit,
        requested,
        allowed,
        retry_after_secs: seconds_until_next_month(),
    })
}
