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

    /// Hosted Creem checkout for a fixed wallet top-up pack (metadata purpose=wallet_topup).
    pub async fn client_create_topup_checkout(
        &self,
        user_id: UserId,
        pack: &app_core::TopupPack,
        product_id: &str,
    ) -> Result<CheckoutSession, ProviderError> {
        let (url, session_id) = self
            .client
            .create_topup_checkout_session(product_id, user_id, pack.id, pack.amount_fen)
            .await
            .map_err(|error| ProviderError::Request(error.to_string()))?;
        Ok(CheckoutSession::Url { url, session_id })
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
            "subscription.paid" | "checkout.completed" | "payment.succeeded" => {
                let data = value
                    .get("data")
                    .ok_or_else(|| ProviderError::Invalid("missing data field".to_string()))?;
                if is_wallet_topup_metadata(data) {
                    parse_wallet_topup(data, &event_id)?
                } else if event_type == "subscription.paid" {
                    parse_subscription_paid(data)?
                } else {
                    // One-time payment events without top-up purpose have no product effect yet.
                    ProviderEvent::Ignored
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

fn is_wallet_topup_metadata(data: &serde_json::Value) -> bool {
    data.get("purpose")
        .or_else(|| data.pointer("/metadata/purpose"))
        .and_then(|v| v.as_str())
        .map(|s| s.eq_ignore_ascii_case(app_core::PRODUCT_KIND_WALLET_TOPUP))
        .unwrap_or(false)
}

fn parse_wallet_topup(
    data: &serde_json::Value,
    event_id: &str,
) -> Result<ProviderEvent, ProviderError> {
    let user_id = data
        .get("user_id")
        .or_else(|| data.pointer("/metadata/user_id"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ProviderError::Invalid("missing user_id for wallet_topup".to_string()))?
        .to_string();
    let pack_id = data
        .get("pack_id")
        .or_else(|| data.pointer("/metadata/pack_id"))
        .or_else(|| data.get("plan_id"))
        .or_else(|| data.pointer("/metadata/plan_id"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ProviderError::Invalid("missing pack_id for wallet_topup".to_string()))?
        .to_string();
    let amount_fen = data
        .get("amount_fen")
        .or_else(|| data.pointer("/metadata/amount_fen"))
        .and_then(|v| v.as_i64())
        .or_else(|| {
            app_core::topup_pack_by_id(&pack_id).map(|p| p.amount_fen)
        })
        .ok_or_else(|| {
            ProviderError::Invalid("missing amount_fen for wallet_topup".to_string())
        })?;
    if amount_fen <= 0 {
        return Err(ProviderError::Invalid(
            "invalid amount_fen for wallet_topup".to_string(),
        ));
    }
    let provider_order_id = string_or_nested(data, "id", "subscription_id")
        .or_else(|_| string_or_nested(data, "checkout_id", "order_id"))
        .unwrap_or_else(|_| event_id.to_string());
    Ok(ProviderEvent::WalletTopupPaid {
        user_id,
        pack_id,
        amount_fen,
        provider_order_id,
        event_id: event_id.to_string(),
    })
}

fn parse_subscription_paid(data: &serde_json::Value) -> Result<ProviderEvent, ProviderError> {
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
    Ok(ProviderEvent::SubscriptionPaid {
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
    })
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
