use super::{CheckoutSession, PaymentProvider, ProviderError, ProviderWebhook};
use crate::types::BillingConfig;
use crate::CreemClient;
use app_core::{BillingProvider, ProviderEvent};
use chrono::TimeZone;
use common::UserId;
use hmac::Mac;

/// Creem adapter — JSON webhooks signed with HMAC-SHA256 (`creem-signature`
/// header, hex-encoded over the raw body) and hosted checkout URLs.
pub struct CreemAdapter {
    config: BillingConfig,
    client: CreemClient,
}

impl CreemAdapter {
    pub fn new(config: BillingConfig) -> Self {
        let client = CreemClient::new(config.clone());
        Self { config, client }
    }

    fn verify_signature(&self, signature: Option<&str>, payload: &[u8]) -> Result<(), ProviderError> {
        let mut mac = match crate::types::HmacSha256::new_from_slice(
            self.config.creem_webhook_secret.as_bytes(),
        ) {
            Ok(m) => m,
            Err(error) => {
                return Err(ProviderError::Invalid(format!(
                    "invalid HMAC key: {error}"
                )));
            }
        };
        mac.update(payload);
        let expected_sig = hex::encode(mac.finalize().into_bytes());
        if signature.unwrap_or_default() != expected_sig {
            return Err(ProviderError::Signature(
                "invalid Creem signature".to_string(),
            ));
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl PaymentProvider for CreemAdapter {
    fn id(&self) -> BillingProvider {
        BillingProvider::Creem
    }

    async fn create_checkout(
        &self,
        user_id: UserId,
        plan_id: &str,
        _order_ref: &str,
    ) -> Result<CheckoutSession, ProviderError> {
        let Some(product_id) = self
            .config
            .creem_checkout_product_for_plan(plan_id)
            .map(str::to_string)
        else {
            return Err(ProviderError::Request(format!(
                "requested billing plan is not configured for checkout: {plan_id}"
            )));
        };
        let (url, session_id) = self
            .client
            .create_checkout_session(&product_id, user_id, plan_id)
            .await
            .map_err(|error| ProviderError::Request(error.to_string()))?;
        Ok(CheckoutSession::Url { url, session_id })
    }

    async fn parse_event(
        &self,
        signature: Option<&str>,
        payload: &[u8],
    ) -> Result<ProviderWebhook, ProviderError> {
        self.verify_signature(signature, payload)?;

        let value: serde_json::Value = serde_json::from_slice(payload)
            .map_err(|error| ProviderError::Invalid(error.to_string()))?;
        let event_id = value
            .get("id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ProviderError::Invalid("missing event id".to_string()))?
            .to_string();

        let event_type = value
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        let event = match event_type {
            "subscription.paid" => {
                let data = value
                    .get("data")
                    .ok_or_else(|| ProviderError::Invalid("missing data field".to_string()))?;
                let subscription_id = string_or_nested(data, "id", "subscription_id")?;
                let user_id = data
                    .get("user_id")
                    .or_else(|| data.pointer("/metadata/user_id"))
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| ProviderError::Invalid("missing user_id".to_string()))?
                    .to_string();
                let plan_id = data
                    .get("plan_id")
                    .or_else(|| data.pointer("/metadata/plan_id"))
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| ProviderError::Invalid("missing plan_id".to_string()))?
                    .to_string();
                let price_id = data
                    .get("price_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let amount_cents = data
                    .get("amount")
                    .or_else(|| data.get("amount_cents"))
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| {
                        ProviderError::Invalid("missing amount for subscription.paid".to_string())
                    })?;
                let currency = data
                    .get("currency")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| {
                        ProviderError::Invalid("missing currency for subscription.paid".to_string())
                    })?
                    .to_string();
                ProviderEvent::SubscriptionPaid {
                    subscription_id,
                    user_id,
                    plan_id,
                    price_id,
                    amount_cents,
                    currency,
                    current_period_start: data
                        .get("current_period_start")
                        .and_then(|v| v.as_i64())
                        .and_then(|ts| chrono::Utc.timestamp_opt(ts, 0).single()),
                    current_period_end: data
                        .get("current_period_end")
                        .and_then(|v| v.as_i64())
                        .and_then(|ts| chrono::Utc.timestamp_opt(ts, 0).single()),
                }
            }
            "subscription.canceled" => {
                let data = value
                    .get("data")
                    .ok_or_else(|| ProviderError::Invalid("missing data field".to_string()))?;
                ProviderEvent::SubscriptionCanceled {
                    subscription_id: string_or_nested(data, "id", "subscription_id")?,
                }
            }
            _ => ProviderEvent::Ignored,
        };

        Ok(ProviderWebhook { event_id, event })
    }
}

fn string_or_nested(value: &serde_json::Value, key: &str, alt: &str) -> Result<String, ProviderError> {
    value
        .get(key)
        .or_else(|| value.get(alt))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ProviderError::Invalid(format!("missing {key} field")))
}
