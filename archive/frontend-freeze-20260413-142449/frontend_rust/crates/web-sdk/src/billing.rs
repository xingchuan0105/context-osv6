//! Billing API client
#![allow(dead_code)]

use crate::{ApiClient, dtos::*};
use anyhow::{anyhow, bail};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ApiErrorEnvelope {
    message: String,
}

#[derive(Debug, Deserialize)]
struct ApiEnvelope<T> {
    #[serde(default)]
    ok: bool,
    data: Option<T>,
    error: Option<ApiErrorEnvelope>,
}

#[derive(Debug, Deserialize)]
struct RawPlanQuota {
    metric_type: String,
    soft_limit: Option<i64>,
    hard_limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RawPlanRow {
    plan_id: String,
    name: String,
    description: String,
    price_label: String,
    interval: String,
    checkout_available: bool,
    current: bool,
    quotas: Vec<RawPlanQuota>,
}

#[derive(Debug, Deserialize)]
struct RawPlansPayload {
    plans: Vec<RawPlanRow>,
    current_plan_id: String,
}

#[derive(Debug, Deserialize)]
struct RawSubscription {
    plan_id: String,
    status: String,
    current_period_end: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawSubscriptionPayload {
    subscription: RawSubscription,
}

#[derive(Debug, Deserialize)]
struct RawUsagePayload {
    usage: std::collections::HashMap<String, i64>,
}

#[derive(Debug, Deserialize)]
struct RawCheckoutResponse {
    url: String,
    session_id: String,
}

#[derive(Debug, Deserialize)]
struct RawPortalResponse {
    url: String,
}

fn unwrap_api_data<T>(envelope: ApiEnvelope<T>, fallback: &str) -> anyhow::Result<T> {
    if envelope.ok {
        return envelope
            .data
            .ok_or_else(|| anyhow!("missing data in API response"));
    }

    let message = envelope
        .error
        .map(|err| err.message)
        .unwrap_or_else(|| fallback.to_string());
    bail!(message)
}

fn parse_price_to_cents(label: &str) -> i64 {
    let numeric = label
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect::<String>();
    let amount = numeric.parse::<f64>().unwrap_or(0.0);
    (amount * 100.0).round() as i64
}

fn quota_feature(quota: &RawPlanQuota) -> String {
    let limit = quota.hard_limit.or(quota.soft_limit);
    match limit {
        Some(value) => format!("{}: {}", quota.metric_type, value),
        None => format!("{}: unlimited", quota.metric_type),
    }
}

impl ApiClient {
    /// GET /api/v1/billing/plans
    pub async fn list_plans(&self) -> anyhow::Result<PlansResponse> {
        let envelope: ApiEnvelope<RawPlansPayload> = self.get("/api/v1/billing/plans").await?;
        let payload = unwrap_api_data(envelope, "failed to load billing plans")?;
        let plans = payload
            .plans
            .into_iter()
            .map(|plan| PlanRow {
                id: plan.plan_id,
                name: plan.name,
                price: parse_price_to_cents(&plan.price_label),
                features: if plan.quotas.is_empty() {
                    vec![plan.description]
                } else {
                    plan.quotas.iter().map(quota_feature).collect()
                },
            })
            .collect();
        Ok(PlansResponse { plans })
    }

    /// GET /api/v1/billing/usage
    pub async fn get_usage(&self) -> anyhow::Result<UsageResponse> {
        let envelope: ApiEnvelope<RawUsagePayload> = self.get("/api/v1/billing/usage").await?;
        let payload = unwrap_api_data(envelope, "failed to load billing usage")?;
        let used_tokens = payload.usage.get("embedding_tokens").copied().unwrap_or(0)
            + payload.usage.get("llm_input_tokens").copied().unwrap_or(0)
            + payload.usage.get("llm_output_tokens").copied().unwrap_or(0);
        Ok(UsageResponse {
            used_tokens,
            limit_tokens: 0,
            used_documents: payload.usage.get("pages_processed").copied().unwrap_or(0),
            limit_documents: 0,
        })
    }

    /// GET /api/v1/billing/subscription
    pub async fn get_subscription(&self) -> anyhow::Result<SubscriptionResponse> {
        let envelope: ApiEnvelope<RawSubscriptionPayload> =
            self.get("/api/v1/billing/subscription").await?;
        let payload = unwrap_api_data(envelope, "failed to load billing subscription")?;
        Ok(SubscriptionResponse {
            plan_id: payload.subscription.plan_id,
            status: payload.subscription.status,
            current_period_end: payload.subscription.current_period_end.unwrap_or_default(),
        })
    }

    /// POST /api/v1/billing/checkout-session
    pub async fn create_checkout_session(
        &self,
        plan_id: &str,
    ) -> anyhow::Result<serde_json::Value> {
        #[derive(serde::Serialize)]
        struct Body {
            plan_id: String,
        }
        let envelope: ApiEnvelope<RawCheckoutResponse> = self
            .post(
                "/api/v1/billing/checkout-session",
                &Body {
                    plan_id: plan_id.to_string(),
                },
            )
            .await?;
        let payload = unwrap_api_data(envelope, "failed to create checkout session")?;
        Ok(serde_json::json!({
            "url": payload.url,
            "session_id": payload.session_id,
        }))
    }

    /// POST /api/v1/billing/portal-session
    pub async fn create_portal_session(&self) -> anyhow::Result<serde_json::Value> {
        let envelope: ApiEnvelope<RawPortalResponse> = self
            .post("/api/v1/billing/portal-session", &EmptyResponse {})
            .await?;
        let payload = unwrap_api_data(envelope, "failed to create billing portal")?;
        Ok(serde_json::json!({ "url": payload.url }))
    }
}
