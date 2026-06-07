use anyhow::Result;
use avrag_storage_pg::PgAppRepository;
use common::{ApiResponse, UserId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use hmac::Mac;

use crate::core::{
    build_plan_payloads, current_metric_usage, ensure_customer,
    get_current_subscription, load_customer_id, load_plan_quotas, load_quota_limit, load_usage,
    load_usage_history, load_usage_window, process_webhook_event, seconds_until_next_month,
    claim_webhook_with_lease, update_webhook_lease_status,
};
use crate::types::{PLAN_FREE, PLAN_PRO, PLAN_PLUS, BillingProvider, UsageHistoryResponse, UsageWindowResponse};
use crate::{BillingConfig, StripeClient, Subscription, CreemClient, AlipayClient};

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

pub async fn handle_get_plans(
    repo: Arc<PgAppRepository>,
    user_id: UserId,
) -> ApiResponse<serde_json::Value> {
    let config = BillingConfig::from_env();
    let subscription = match get_current_subscription(repo.clone(), user_id).await {
        Ok(sub) => sub,
        Err(error) => return ApiResponse::err("billing_plans_failed", &error.to_string()),
    };
    let current_plan_id = subscription.plan_id.clone();
    let quotas = match load_plan_quotas(repo).await {
        Ok(quotas) => quotas,
        Err(error) => return ApiResponse::err("billing_plans_failed", &error.to_string()),
    };

    let base_plans = build_plan_payloads(&config, &current_plan_id, &quotas);
    let plans: Vec<serde_json::Value> = base_plans
        .into_iter()
        .map(|mut plan| {
            let plan_id = plan.get("plan_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
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

pub async fn handle_get_subscription(
    repo: Arc<PgAppRepository>,
    user_id: UserId,
) -> ApiResponse<SubscriptionResponse> {
    match get_current_subscription(repo, user_id).await {
        Ok(subscription) => ApiResponse::ok(SubscriptionResponse { subscription }),
        Err(error) => ApiResponse::err("billing_subscription_failed", &error.to_string()),
    }
}

pub async fn handle_get_usage(
    repo: Arc<PgAppRepository>,
    user_id: UserId,
) -> ApiResponse<UsageResponse> {
    match load_usage(repo, user_id).await {
        Ok(usage) => ApiResponse::ok(UsageResponse { usage }),
        Err(error) => ApiResponse::err("billing_usage_failed", &error.to_string()),
    }
}

pub async fn handle_get_usage_window(
    repo: Arc<PgAppRepository>,
    user_id: UserId,
) -> ApiResponse<UsageWindowResponse> {
    match load_usage_window(repo, user_id).await {
        Ok(window) => ApiResponse::ok(window),
        Err(error) => ApiResponse::err("billing_usage_window_failed", &error.to_string()),
    }
}

pub async fn handle_get_usage_history(
    repo: Arc<PgAppRepository>,
    user_id: UserId,
    days: i32,
) -> ApiResponse<UsageHistoryResponse> {
    match load_usage_history(repo, user_id, days).await {
        Ok(history) => ApiResponse::ok(history),
        Err(error) => ApiResponse::err("billing_usage_history_failed", &error.to_string()),
    }
}

pub async fn handle_create_checkout(
    repo: Arc<PgAppRepository>,
    user_id: UserId,
    body: CreateCheckoutRequest,
) -> ApiResponse<CheckoutResponse> {
    let config = BillingConfig::from_env();
    let requested_plan = body.plan_id.as_deref().unwrap_or(PLAN_PRO).trim();
    if requested_plan == PLAN_FREE {
        return ApiResponse::err(
            "billing_plan_not_checkoutable",
            "free plan does not require checkout",
        );
    }

    let requested_provider = body.provider.unwrap_or(BillingProvider::Stripe);

    match requested_provider {
        BillingProvider::Stripe => {
            if !config.stripe_enabled() {
                return ApiResponse::err("billing_unconfigured", "Stripe billing checkout is not configured");
            }
            let client = StripeClient::new(config.clone());
            let Some(price_id) = config
                .checkout_price_for_plan(requested_plan)
                .map(str::to_string)
            else {
                return ApiResponse::err(
                    "invalid_billing_plan",
                    "requested billing plan is not configured for checkout",
                );
            };

            match ensure_customer(repo.clone(), &client, user_id).await {
                Ok(customer_id) => {
                    match client
                        .create_checkout_session(&customer_id, &price_id, user_id, requested_plan)
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
                Err(error) => ApiResponse::err("billing_customer_failed", &error.to_string()),
            }
        }
        BillingProvider::Creem => {
            if !config.creem_enabled() {
                return ApiResponse::err("billing_unconfigured", "Creem billing checkout is not configured");
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
            let client = CreemClient::new(config.clone());
            match client.create_checkout_session(&product_id, user_id, requested_plan).await {
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
                return ApiResponse::err("billing_unconfigured", "Alipay billing checkout is not configured");
            }
            let amount_cents = match requested_plan {
                PLAN_PRO => 2000,
                PLAN_PLUS => 10000,
                _ => {
                    return ApiResponse::err(
                        "invalid_billing_plan",
                        "requested billing plan is not supported",
                    );
                }
            };
            let amount_str = config.alipay_checkout_price_for_plan(requested_plan)
                .unwrap_or(if requested_plan == PLAN_PRO { "20.00" } else { "100.00" });

            let out_trade_no = uuid::Uuid::new_v4().to_string();

            // Insert pending order in transaction
            let mut tx = match repo.raw().begin().await {
                Ok(tx) => tx,
                Err(error) => return ApiResponse::err("billing_checkout_failed", &error.to_string()),
            };
            if let Err(error) = sqlx::query("select set_config('app.current_role', 'super_admin', true)").execute(tx.as_mut()).await {
                return ApiResponse::err("billing_checkout_failed", &error.to_string());
            }
            let insert_res = sqlx::query(
                r#"
                insert into billing_orders (user_id, provider, provider_order_id, plan_id, status, amount_cents, currency)
                values ($1, 'alipay', $2, $3, 'pending', $4, 'CNY')
                "#,
            )
            .bind(user_id.into_uuid())
            .bind(&out_trade_no)
            .bind(requested_plan)
            .bind(amount_cents)
            .execute(tx.as_mut())
            .await;

            if let Err(error) = insert_res {
                return ApiResponse::err("billing_checkout_failed", &error.to_string());
            }
            if let Err(error) = tx.commit().await {
                return ApiResponse::err("billing_checkout_failed", &error.to_string());
            }

            let notify_url = config.alipay_notify_url.clone().unwrap_or_else(|| {
                format!(
                    "{}/webhooks/alipay",
                    std::env::var("AVRAG_PUBLIC_BASE_URL")
                        .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string())
                )
            });

            let client = AlipayClient::new(config.clone());
            let subject = format!("Context OS - {} Subscription", requested_plan);
            match client.create_precreate_order(amount_str, &subject, &out_trade_no, &notify_url).await {
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

pub async fn handle_create_portal(
    repo: Arc<PgAppRepository>,
    user_id: UserId,
) -> ApiResponse<PortalResponse> {
    let config = BillingConfig::from_env();
    let client = StripeClient::new(config.clone());
    if !config.stripe_enabled() {
        return ApiResponse::err("billing_unconfigured", "billing portal is not configured");
    }
    match load_customer_id(repo, user_id).await {
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
                if let Some(a) = h1 { res.push(a); }
                if let Some(b) = h2 { res.push(b); }
            }
        } else if c == '+' {
            res.push(' ');
        } else {
            res.push(c);
        }
    }
    res
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

pub async fn handle_webhook(
    repo: Arc<PgAppRepository>,
    provider: BillingProvider,
    signature: Option<&str>,
    payload: &[u8],
) -> ApiResponse<serde_json::Value> {
    let config = BillingConfig::from_env();

    // 1. Verify signatures
    match provider {
        BillingProvider::Stripe => {
            if !config.webhook_enabled() {
                return ApiResponse::err("billing_unconfigured", "billing webhook is not configured");
            }
            let client = StripeClient::new(config.clone());
            if let Err(error) = client.verify_webhook_signature(payload, signature.unwrap_or_default()) {
                return ApiResponse::err("billing_webhook_signature_failed", &error.to_string());
            }
        }
        BillingProvider::Creem => {
            let secret = std::env::var("CREEM_WEBHOOK_SECRET").unwrap_or_default();
            let mut mac = match crate::types::HmacSha256::new_from_slice(secret.as_bytes()) {
                Ok(m) => m,
                Err(error) => {
                    return ApiResponse::err("billing_webhook_failed", &format!("invalid HMAC key: {error}"));
                }
            };
            mac.update(payload);
            let expected_sig = hex::encode(mac.finalize().into_bytes());
            if signature.unwrap_or_default() != expected_sig {
                return ApiResponse::err("billing_webhook_signature_failed", "invalid Creem signature");
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
            let sign = params.iter()
                .find(|(k, _)| k == "sign")
                .map(|(_, v)| v.as_str())
                .unwrap_or_default();
            if sign.is_empty() {
                return ApiResponse::err("billing_webhook_signature_failed", "missing Alipay signature");
            }
            let client = AlipayClient::new(config.clone());
            if let Err(error) = client.verify_signature(&params, sign) {
                return ApiResponse::err("billing_webhook_signature_failed", &error.to_string());
            }
        }
    }

    // 2. Parse payload to JSON and extract event_id
    let (json, event_id) = match provider {
        BillingProvider::Stripe | BillingProvider::Creem => {
            let val: serde_json::Value = match serde_json::from_slice(payload) {
                Ok(v) => v,
                Err(error) => return ApiResponse::err("billing_webhook_invalid", &error.to_string()),
            };
            let ev_id = val.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            (val, ev_id)
        }
        BillingProvider::Alipay => {
            let val = alipay_payload_to_json(payload);
            let ev_id = val.get("notify_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            (val, ev_id)
        }
    };

    if event_id.is_empty() {
        return ApiResponse::err("billing_webhook_invalid", "missing event/notify id");
    }

    // 3. Lease-based idempotence check
    let claim = match claim_webhook_with_lease(repo.clone(), provider, &event_id).await {
        Ok(claim) => claim,
        Err(error) => return ApiResponse::err("billing_webhook_failed", &error.to_string()),
    };

    if claim.duplicate_processed {
        return ApiResponse::ok(serde_json::json!({
            "status": "ok",
            "duplicate": true,
        }));
    }

    // 4. Process event
    if let Err(error) = process_webhook_event(repo.clone(), provider, &json, &config).await {
        let _ = update_webhook_lease_status(repo, provider, &claim.event_id, "failed", Some(error.to_string())).await;
        return ApiResponse::err("billing_webhook_failed", &error.to_string());
    }

    if let Err(error) = update_webhook_lease_status(repo, provider, &claim.event_id, "processed", None).await {
        return ApiResponse::err("billing_webhook_failed", &error.to_string());
    }

    ApiResponse::ok(serde_json::json!({ "status": "ok" }))
}

pub async fn check_quota(
    repo: Arc<PgAppRepository>,
    user_id: UserId,
    metric_type: &str,
    requested: i64,
) -> Result<QuotaDecision> {
    let subscription = get_current_subscription(repo.clone(), user_id).await?;
    let plan_id = subscription.plan_id;
    let quota = load_quota_limit(repo.clone(), &plan_id, metric_type).await?;
    let current_usage = current_metric_usage(repo, user_id, metric_type).await?;
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
