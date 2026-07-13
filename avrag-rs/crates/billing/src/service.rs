use anyhow::Result;
use app_core::BillingStorePort;
use common::{ApiResponse, UserId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use hmac::Mac;

use crate::core::{
    build_plan_payloads, claim_webhook_with_lease, current_metric_usage, get_current_subscription,
    load_plan_quotas, load_quota_limit, process_webhook_event, seconds_until_next_month,
    update_webhook_lease_status,
};
use crate::types::{BillingProvider, PLAN_FREE, PLAN_PRO};
use crate::{AlipayClient, BillingConfig, CreemClient, Subscription};

#[derive(Deserialize)]
pub struct CreateCheckoutRequest {
    pub plan_id: Option<String>,
    pub provider: Option<BillingProvider>,
}

#[derive(Serialize)]
pub struct CheckoutResponse {
    pub url: String,
    pub session_id: String,
    pub qr_code: Option<String>,
    pub order_id: Option<String>,
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

pub struct BillingService {
    config: BillingConfig,
    creem: CreemClient,
    alipay: AlipayClient,
}

static BILLING_SERVICE: LazyLock<BillingService> = LazyLock::new(BillingService::from_env);

impl BillingService {
    pub fn from_env() -> Self {
        let config = BillingConfig::from_env();
        let creem = CreemClient::new(config.clone());
        let alipay = AlipayClient::new(config.clone());
        Self {
            config,
            creem,
            alipay,
        }
    }

    pub fn shared() -> &'static Self {
        &BILLING_SERVICE
    }

    pub fn config(&self) -> &BillingConfig {
        &self.config
    }

    pub async fn get_plans(
        &self,
        store: Arc<dyn BillingStorePort>,
        user_id: UserId,
    ) -> ApiResponse<serde_json::Value> {
        let config = &self.config;
        let subscription = match get_current_subscription(store.clone(), user_id).await {
            Ok(sub) => sub,
            Err(error) => return ApiResponse::err("billing_plans_failed", &error.to_string()),
        };
        let current_plan_id = subscription.plan_id.clone();
        let quotas = match load_plan_quotas(store).await {
            Ok(quotas) => quotas,
            Err(error) => return ApiResponse::err("billing_plans_failed", &error.to_string()),
        };

        let base_plans = build_plan_payloads(config, &current_plan_id, &quotas);
        let plans: Vec<serde_json::Value> = base_plans
            .into_iter()
            .map(|mut plan| {
                let plan_id = plan
                    .get("plan_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let obj = plan.as_object_mut().expect("plan is a JSON object");
                obj.insert(
                    "price_label_cny".to_string(),
                    serde_json::Value::String(config.price_label_cny_for_plan(&plan_id)),
                );
                obj.insert(
                    "price_label_usd".to_string(),
                    serde_json::Value::String(config.price_label_usd_for_plan(&plan_id)),
                );
                plan
            })
            .collect();

        ApiResponse::ok(serde_json::json!({
            "plans": plans,
            "current_plan_id": current_plan_id,
        }))
    }

    /// External customer portal is **not** offered (Stripe removed; Creem/Alipay
    /// use in-app plan change + `/pricing` checkout). Always returns unavailable.
    pub async fn create_portal(
        &self,
        _store: Arc<dyn BillingStorePort>,
        _user_id: UserId,
    ) -> ApiResponse<PortalResponse> {
        ApiResponse::err(
            "billing_portal_unavailable",
            "External billing portal is not used; change plans via in-app pricing (Creem/Alipay)",
        )
    }

    pub async fn create_checkout(
        &self,
        store: Arc<dyn BillingStorePort>,
        user_id: UserId,
        body: CreateCheckoutRequest,
    ) -> ApiResponse<CheckoutResponse> {
        let config = &self.config;
        let requested_plan = body.plan_id.as_deref().unwrap_or(PLAN_PRO).trim();
        if requested_plan == PLAN_FREE {
            return ApiResponse::err(
                "billing_plan_not_checkoutable",
                "free plan does not require checkout",
            );
        }

        let requested_provider = body
            .provider
            .unwrap_or_else(|| config.default_checkout_provider());

        match requested_provider {
            BillingProvider::Stripe => ApiResponse::err(
                "billing_provider_removed",
                "Stripe is not a product payment provider; use Creem (international) or Alipay (China)",
            ),
            BillingProvider::Creem => {
                if !config.creem_enabled() {
                    return ApiResponse::err(
                        "billing_unconfigured",
                        "Creem billing checkout is not configured",
                    );
                }
                let Some(product_id) = config
                    .creem_checkout_product_for_plan(requested_plan)
                    .map(str::to_string)
                else {
                    return ApiResponse::err(
                        "invalid_billing_plan",
                        "requested billing plan is not configured for checkout",
                    );
                };
                match self
                    .creem
                    .create_checkout_session(&product_id, user_id, requested_plan)
                    .await
                {
                    Ok((url, session_id)) => ApiResponse::ok(CheckoutResponse {
                        url,
                        session_id,
                        qr_code: None,
                        order_id: None,
                    }),
                    Err(error) => ApiResponse::err("billing_checkout_failed", &error.to_string()),
                }
            }
            BillingProvider::Alipay => {
                if !config.alipay_enabled() {
                    return ApiResponse::err(
                        "billing_unconfigured",
                        "Alipay billing checkout is not configured",
                    );
                }
                let Some(amount_str) = config
                    .alipay_checkout_price_for_plan(requested_plan)
                    .map(str::to_string)
                else {
                    return ApiResponse::err(
                        "invalid_billing_plan",
                        "requested billing plan is not configured for Alipay checkout",
                    );
                };
                let amount_cents = BillingConfig::decimal_price_to_cents(&amount_str);
                if amount_cents <= 0 {
                    return ApiResponse::err(
                        "invalid_billing_plan",
                        "Alipay price for requested plan is invalid",
                    );
                }

                let out_trade_no = uuid::Uuid::new_v4().to_string();

                if let Err(error) = store
                    .insert_pending_alipay_order(
                        user_id,
                        &out_trade_no,
                        requested_plan,
                        amount_cents,
                    )
                    .await
                {
                    return ApiResponse::err("billing_checkout_failed", &error.to_string());
                }

                let notify_url = config.alipay_notify_url.clone().unwrap_or_else(|| {
                    format!(
                        "{}/webhooks/alipay",
                        std::env::var("AVRAG_PUBLIC_BASE_URL")
                            .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string())
                    )
                });

                let subject = format!("Context OS - {} Subscription", requested_plan);
                match self
                    .alipay
                    .create_precreate_order(&amount_str, &subject, &out_trade_no, &notify_url)
                    .await
                {
                    Ok(qr_code) => ApiResponse::ok(CheckoutResponse {
                        url: "".to_string(),
                        session_id: "".to_string(),
                        qr_code: Some(qr_code),
                        order_id: Some(out_trade_no),
                    }),
                    Err(error) => ApiResponse::err("billing_checkout_failed", &error.to_string()),
                }
            }
        }
    }

    pub async fn handle_webhook(
        &self,
        store: Arc<dyn BillingStorePort>,
        provider: BillingProvider,
        signature: Option<&str>,
        payload: &[u8],
    ) -> ApiResponse<serde_json::Value> {
        let config = &self.config;

        // 1. Verify signatures (Stripe provider permanently rejected).
        match provider {
            BillingProvider::Stripe => {
                return ApiResponse::err(
                    "billing_provider_removed",
                    "Stripe webhooks are no longer accepted; product billing is Creem + Alipay only",
                );
            }
            BillingProvider::Creem => {
                let secret = std::env::var("CREEM_WEBHOOK_SECRET").unwrap_or_default();
                let mut mac = match crate::types::HmacSha256::new_from_slice(secret.as_bytes()) {
                    Ok(m) => m,
                    Err(error) => {
                        return ApiResponse::err(
                            "billing_webhook_failed",
                            &format!("invalid HMAC key: {error}"),
                        );
                    }
                };
                mac.update(payload);
                let expected_sig = hex::encode(mac.finalize().into_bytes());
                if signature.unwrap_or_default() != expected_sig {
                    return ApiResponse::err(
                        "billing_webhook_signature_failed",
                        "invalid Creem signature",
                    );
                }
            }
            BillingProvider::Alipay => {
                let query_str = String::from_utf8_lossy(payload);
                let mut params = Vec::new();
                for part in query_str.split('&') {
                    if let Some((k, v)) = part.split_once('=') {
                        params.push((percent_decode(k), percent_decode(v)));
                    }
                }
                let sign = params
                    .iter()
                    .find(|(k, _)| k == "sign")
                    .map(|(_, v)| v.as_str())
                    .unwrap_or_default();
                if sign.is_empty() {
                    return ApiResponse::err(
                        "billing_webhook_signature_failed",
                        "missing Alipay signature",
                    );
                }
                if let Err(error) = self.alipay.verify_signature(&params, sign) {
                    return ApiResponse::err("billing_webhook_signature_failed", &error.to_string());
                }
            }
        }

        // 2. Parse payload to JSON and extract event_id
        let (json, event_id) = match provider {
            BillingProvider::Stripe | BillingProvider::Creem => {
                let val: serde_json::Value = match serde_json::from_slice(payload) {
                    Ok(v) => v,
                    Err(error) => {
                        return ApiResponse::err("billing_webhook_invalid", &error.to_string());
                    }
                };
                let ev_id = val
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                (val, ev_id)
            }
            BillingProvider::Alipay => {
                let val = alipay_payload_to_json(payload);
                let ev_id = val
                    .get("notify_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                (val, ev_id)
            }
        };

        if event_id.is_empty() {
            return ApiResponse::err("billing_webhook_invalid", "missing event/notify id");
        }

        // 3. Lease-based idempotence check
        let claim = match claim_webhook_with_lease(store.clone(), provider, &event_id).await {
            Ok(claim) => claim,
            Err(error) => return webhook_error_response(error),
        };

        if claim.duplicate_processed {
            return ApiResponse::ok(serde_json::json!({
                "status": "ok",
                "duplicate": true,
            }));
        }

        // 4. Process event
        if let Err(error) = process_webhook_event(store.clone(), provider, &json, config).await {
            let _ = update_webhook_lease_status(
                store,
                provider,
                &claim.event_id,
                "failed",
                Some(error.to_string()),
            )
            .await;
            return webhook_error_response(error);
        }

        if let Err(error) =
            update_webhook_lease_status(store, provider, &claim.event_id, "processed", None).await
        {
            return webhook_error_response(error);
        }

        ApiResponse::ok(serde_json::json!({ "status": "ok" }))
    }

    pub async fn check_quota(
        &self,
        store: Arc<dyn BillingStorePort>,
        user_id: UserId,
        metric_type: &str,
        requested: i64,
    ) -> Result<QuotaDecision> {
        let subscription = get_current_subscription(store.clone(), user_id).await?;
        let plan_id = subscription.plan_id;
        let quota = load_quota_limit(store.clone(), &plan_id, metric_type).await?;
        let current_usage = current_metric_usage(store, user_id, metric_type).await?;
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
}

fn percent_decode(s: &str) -> String {
    let mut res = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h1 = chars.next();
            let h2 = chars.next();
            if let (Some(a), Some(b)) = (h1, h2) {
                if let Ok(hex) = u8::from_str_radix(&format!("{}{}", a, b), 16) {
                    res.push(hex as char);
                } else {
                    res.push('%');
                    res.push(a);
                    res.push(b);
                }
            } else {
                res.push('%');
                if let Some(a) = h1 {
                    res.push(a);
                }
                if let Some(b) = h2 {
                    res.push(b);
                }
            }
        } else if c == '+' {
            res.push(' ');
        } else {
            res.push(c);
        }
    }
    res
}

fn webhook_db_unavailable(error: &anyhow::Error) -> bool {
    let message = error.to_string();
    message.contains("PoolTimedOut")
        || message.contains("PoolClosed")
        || message.contains("connection")
}

fn webhook_error_response(error: anyhow::Error) -> ApiResponse<serde_json::Value> {
    if webhook_db_unavailable(&error) {
        ApiResponse::err(
            "billing_webhook_unavailable",
            "billing database unavailable",
        )
    } else {
        ApiResponse::err("billing_webhook_failed", &error.to_string())
    }
}

fn alipay_payload_to_json(payload: &[u8]) -> serde_json::Value {
    let s = String::from_utf8_lossy(payload);
    let mut map = serde_json::Map::new();
    for part in s.split('&') {
        let mut kv = part.splitn(2, '=');
        if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
            let k_decoded = percent_decode(k);
            let v_decoded = percent_decode(v);
            map.insert(k_decoded, serde_json::Value::String(v_decoded));
        }
    }
    serde_json::Value::Object(map)
}
