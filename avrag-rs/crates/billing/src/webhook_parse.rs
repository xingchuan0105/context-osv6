use anyhow::{Result, anyhow, bail};
use chrono::{DateTime, TimeZone, Utc};

use crate::types::{BillingConfig, StripeSubscriptionSnapshot, STATUS_ACTIVE};

fn string_or_nested_id(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value?;
    if let Some(id) = value.as_str() {
        let id = id.trim();
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }
    value
        .get("id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn unix_timestamp_to_utc(timestamp: Option<i64>) -> Option<DateTime<Utc>> {
    timestamp.and_then(|value| Utc.timestamp_opt(value, 0).single())
}

fn map_stripe_status_to_local(status: &str) -> String {
    match status.trim().to_lowercase().as_str() {
        "active" | "trialing" => STATUS_ACTIVE.to_string(),
        "canceled" | "cancelled" => "canceled".to_string(),
        "past_due" => "past_due".to_string(),
        "unpaid" => "unpaid".to_string(),
        other => other.to_string(),
    }
}

pub fn subscription_snapshot_from_event(
    payload: &serde_json::Value,
    config: &BillingConfig,
) -> Result<StripeSubscriptionSnapshot> {
    let subscription = payload
        .pointer("/data/object")
        .ok_or_else(|| anyhow!("subscription payload is required"))?;

    let stripe_subscription_id = subscription
        .get("id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("subscription payload is missing id"))?
        .to_string();
    let stripe_customer_id = string_or_nested_id(subscription.get("customer")).unwrap_or_default();
    let stripe_price_id = subscription
        .pointer("/items/data/0/price/id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    let mut user_id = subscription
        .pointer("/metadata/user_id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    let mut plan_id = subscription
        .pointer("/metadata/plan_id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    if plan_id.is_empty()
        && let Some(mapped_plan) = config.plan_id_by_price_id(&stripe_price_id)
    {
        plan_id = mapped_plan.to_string();
    }
    if user_id.is_empty() && stripe_customer_id.is_empty() {
        bail!("subscription metadata missing user_id and customer id");
    }
    if plan_id.is_empty() {
        bail!("subscription metadata missing plan_id");
    }

    Ok(StripeSubscriptionSnapshot {
        user_id: std::mem::take(&mut user_id),
        stripe_customer_id,
        stripe_subscription_id,
        stripe_price_id,
        plan_id,
        status: map_stripe_status_to_local(
            subscription
                .get("status")
                .and_then(|value| value.as_str())
                .unwrap_or(STATUS_ACTIVE),
        ),
        current_period_start: unix_timestamp_to_utc(
            subscription
                .get("current_period_start")
                .and_then(|value| value.as_i64()),
        ),
        current_period_end: unix_timestamp_to_utc(
            subscription
                .get("current_period_end")
                .and_then(|value| value.as_i64()),
        ),
        cancel_at_period_end: subscription
            .get("cancel_at_period_end")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
    })
}
