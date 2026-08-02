use super::{CheckoutSession, PaymentProvider, ProviderError, ProviderWebhook};
use crate::service::percent_decode;
use crate::types::BillingConfig;
use crate::AlipayClient;
use app_core::{BillingProvider, ProviderEvent};
use common::UserId;

/// Alipay adapter — F2F (face-to-face) QR checkout plus async-notify webhooks
/// signed with RSA2.
///
/// Notify specifics (Alipay async notify, "支付宝异步通知"):
/// - Body is `application/x-www-form-urlencoded`; every key and value is
///   percent-encoded. Multi-byte UTF-8 values (e.g. a Chinese `subject`) must
///   be byte-decoded — `+` is a space, `%XX` is a byte — *before* both
///   signature verification and JSON conversion.
/// - Verification excludes both `sign` and `sign_type`; request signing
///   excludes only `sign` (see [`AlipayClient::sign`] / `verify_signature`).
/// - `app_id` must match our own (anti cross-merchant replay).
pub struct AlipayAdapter {
    config: BillingConfig,
    client: AlipayClient,
}

impl AlipayAdapter {
    pub fn new(config: BillingConfig) -> Self {
        let client = AlipayClient::new(config.clone());
        Self { config, client }
    }

    /// Percent-decode each `k=v` pair, preserving bytes (CJK-safe).
    fn decode_params(&self, raw: &[u8]) -> Vec<(String, String)> {
        let query_str = String::from_utf8_lossy(raw);
        let mut params = Vec::new();
        for part in query_str.split('&') {
            if let Some((k, v)) = part.split_once('=') {
                params.push((percent_decode(k), percent_decode(v)));
            }
        }
        params
    }

    fn verify_signature(
        &self,
        params: &[(String, String)],
    ) -> Result<(), ProviderError> {
        let sign = params
            .iter()
            .find(|(k, _)| k == "sign")
            .map(|(_, v)| v.as_str())
            .unwrap_or_default();
        if sign.is_empty() {
            return Err(ProviderError::Signature(
                "missing Alipay signature".to_string(),
            ));
        }
        self.client
            .verify_signature(params, sign)
            .map_err(|error| ProviderError::Signature(error.to_string()))
    }
}

#[async_trait::async_trait]
impl PaymentProvider for AlipayAdapter {
    fn id(&self) -> BillingProvider {
        BillingProvider::Alipay
    }

    async fn create_checkout(
        &self,
        _user_id: UserId,
        plan_id: &str,
        order_ref: &str,
    ) -> Result<CheckoutSession, ProviderError> {
        let Some(amount_str) = self
            .config
            .alipay_checkout_price_for_plan(plan_id)
            .map(str::to_string)
        else {
            return Err(ProviderError::Request(format!(
                "requested billing plan is not configured for Alipay checkout: {plan_id}"
            )));
        };
        let notify_url = self.config.alipay_notify_url.clone().unwrap_or_else(|| {
            format!(
                "{}/webhooks/alipay",
                std::env::var("AVRAG_PUBLIC_BASE_URL")
                    .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string())
            )
        });
        let subject = format!("Context OS - {} Subscription", plan_id);
        let qr_code = self
            .client
            .create_precreate_order(&amount_str, &subject, order_ref, &notify_url)
            .await
            .map_err(|error| ProviderError::Request(error.to_string()))?;
        Ok(CheckoutSession::QrCode {
            qr_code,
            order_id: order_ref.to_string(),
        })
    }

    async fn parse_event(
        &self,
        _signature: Option<&str>,
        payload: &[u8],
    ) -> Result<ProviderWebhook, ProviderError> {
        let params = self.decode_params(payload);
        self.verify_signature(&params)?;

        let map: serde_json::Map<String, serde_json::Value> = params
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();
        let value = serde_json::Value::Object(map);

        // The notify must target *this* app (anti spoofing / cross-merchant replay).
        let payload_app_id = value
            .get("app_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if payload_app_id.is_empty() || payload_app_id != self.config.alipay_app_id.trim() {
            return Err(ProviderError::Invalid("alipay notify app_id mismatch".to_string()));
        }

        let event_id = value
            .get("notify_id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ProviderError::Invalid("missing notify_id".to_string()))?
            .to_string();

        let trade_status = value
            .get("trade_status")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        let event = match trade_status {
            "TRADE_SUCCESS" | "TRADE_FINISHED" => {
                let out_trade_no = value
                    .get("out_trade_no")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| ProviderError::Invalid("missing out_trade_no".to_string()))?
                    .to_string();
                let paid_cents = BillingConfig::decimal_price_to_cents(
                    value
                        .get("total_amount")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default(),
                );
                if paid_cents <= 0 {
                    return Err(ProviderError::Invalid(
                        "invalid total_amount in alipay notify".to_string(),
                    ));
                }
                ProviderEvent::AlipayOrderPaid {
                    out_trade_no,
                    paid_cents,
                }
            }
            _ => ProviderEvent::Ignored,
        };

        Ok(ProviderWebhook { event_id, event })
    }
}
