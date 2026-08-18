use anyhow::Result;
use app_core::BillingStorePort;
use common::{ApiResponse, UserId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use crate::core::{
    build_plan_payloads, claim_webhook_with_lease, current_metric_usage, get_current_subscription,
    load_plan_quotas, load_quota_limit, process_webhook_event, seconds_until_next_month,
    update_webhook_lease_status,
};
use crate::payment_provider::{
    AlipayAdapter, CheckoutSession, CreemAdapter, PaymentProvider, ProviderError,
};
use crate::types::{BillingProvider, PLAN_FREE, PLAN_PRO};
use crate::{BillingConfig, Subscription};

#[derive(Deserialize)]
pub struct CreateCheckoutRequest {
    /// Subscription plan id when `kind` is subscription (default).
    pub plan_id: Option<String>,
    pub provider: Option<BillingProvider>,
    /// `subscription` (default) or `wallet_topup` (ADR-0010 PR5).
    pub kind: Option<String>,
    /// Required when `kind = wallet_topup` (`topup_50` / `topup_100` / `topup_200`).
    pub topup_pack_id: Option<String>,
}

#[derive(Serialize)]
pub struct CheckoutResponse {
    pub url: String,
    pub session_id: String,
    pub qr_code: Option<String>,
    pub order_id: Option<String>,
}

#[derive(Serialize)]
pub struct OrderStatusResponse {
    pub order_id: String,
    pub status: String,
    pub plan_id: String,
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
    creem: CreemAdapter,
    alipay: AlipayAdapter,
}

static BILLING_SERVICE: LazyLock<BillingService> = LazyLock::new(BillingService::from_env);

impl BillingService {
    pub fn from_env() -> Self {
        let config = BillingConfig::from_env();
        let creem = CreemAdapter::new(config.clone());
        let alipay = AlipayAdapter::new(config.clone());
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

    /// Route a provider id to its registered adapter (Creem / Alipay). Stripe is
    /// rejected by callers before this lookup; there is deliberately no Stripe
    /// adapter. Returns an explicit [`ProviderError::Unsupported`] instead of
    /// panicking when a provider has no adapter (P0-2 — webhook/checkout hot
    /// paths must not carry a future-triggerable abort).
    fn adapter_for(
        &self,
        provider: BillingProvider,
    ) -> Result<&dyn PaymentProvider, ProviderError> {
        let adapters: [&dyn PaymentProvider; 2] = [&self.creem, &self.alipay];
        adapters
            .into_iter()
            .find(|adapter| adapter.id() == provider)
            .ok_or_else(|| ProviderError::Unsupported(provider.to_string()))
    }

    /// Checkout-surface adapter lookup (P3-2): same routing as [`Self::adapter_for`]
    /// with the error already mapped to the checkout error response, so the four
    /// call sites share one copy of the `match { Ok, Err → return }` boilerplate.
    fn checkout_adapter(
        &self,
        provider: BillingProvider,
    ) -> Result<&dyn PaymentProvider, ApiResponse<CheckoutResponse>> {
        self.adapter_for(provider)
            .map_err(|error| ApiResponse::err("billing_checkout_failed", &error.to_string()))
    }

    /// Webhook-surface adapter lookup (P3-2): error pre-mapped through
    /// [`webhook_error_response`], same as the former inline `Err` arms.
    fn webhook_adapter(
        &self,
        provider: BillingProvider,
    ) -> Result<&dyn PaymentProvider, ApiResponse<serde_json::Value>> {
        self.adapter_for(provider)
            .map_err(|error| webhook_error_response(error.into()))
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
        let kind = body
            .kind
            .as_deref()
            .unwrap_or(app_core::CHECKOUT_KIND_SUBSCRIPTION)
            .trim();
        if kind.eq_ignore_ascii_case(app_core::CHECKOUT_KIND_WALLET_TOPUP) {
            return self
                .create_wallet_topup_checkout(store, user_id, body)
                .await;
        }
        if !kind.is_empty() && !kind.eq_ignore_ascii_case(app_core::CHECKOUT_KIND_SUBSCRIPTION) {
            return ApiResponse::err(
                "billing_checkout_kind_invalid",
                "kind must be subscription or wallet_topup",
            );
        }

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
                let adapter = match self.checkout_adapter(requested_provider) {
                    Ok(adapter) => adapter,
                    Err(response) => return response,
                };
                match adapter.create_checkout(user_id, requested_plan, "").await {
                    Ok(CheckoutSession::Url { url, session_id }) => {
                        ApiResponse::ok(CheckoutResponse {
                            url,
                            session_id,
                            qr_code: None,
                            order_id: None,
                        })
                    }
                    Ok(CheckoutSession::QrCode { .. }) => ApiResponse::err(
                        "billing_checkout_failed",
                        "Creem checkout returned an unexpected QR session",
                    ),
                    Err(ProviderError::Request(error)) => {
                        ApiResponse::err("billing_checkout_failed", &error)
                    }
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
                let amount_cents = config
                    .alipay_checkout_price_for_plan(requested_plan)
                    .map(|p| BillingConfig::decimal_price_to_cents(&p))
                    .unwrap_or(0);
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
                        app_core::PRODUCT_KIND_SUBSCRIPTION,
                    )
                    .await
                {
                    return ApiResponse::err("billing_checkout_failed", &error.to_string());
                }

                let adapter = match self.checkout_adapter(requested_provider) {
                    Ok(adapter) => adapter,
                    Err(response) => return response,
                };
                match adapter.create_checkout(user_id, requested_plan, &out_trade_no).await {
                    Ok(CheckoutSession::QrCode { qr_code, order_id }) => {
                        ApiResponse::ok(CheckoutResponse {
                            url: "".to_string(),
                            session_id: "".to_string(),
                            qr_code: Some(qr_code),
                            order_id: Some(order_id),
                        })
                    }
                    Ok(CheckoutSession::Url { .. }) => ApiResponse::err(
                        "billing_checkout_failed",
                        "Alipay checkout returned an unexpected URL session",
                    ),
                    Err(ProviderError::Request(error)) => {
                        ApiResponse::err("billing_checkout_failed", &error)
                    }
                    Err(error) => ApiResponse::err("billing_checkout_failed", &error.to_string()),
                }
            }
        }
    }

    /// Wallet top-up checkout (Creem hosted URL or Alipay F2F QR).
    async fn create_wallet_topup_checkout(
        &self,
        store: Arc<dyn BillingStorePort>,
        user_id: UserId,
        body: CreateCheckoutRequest,
    ) -> ApiResponse<CheckoutResponse> {
        let pack_id = body
            .topup_pack_id
            .as_deref()
            .or(body.plan_id.as_deref())
            .unwrap_or("")
            .trim();
        let Some(pack) = app_core::topup_pack_by_id(pack_id) else {
            return ApiResponse::err(
                "billing_topup_pack_invalid",
                "unknown top-up pack; use topup_50, topup_100, or topup_200",
            );
        };

        let config = &self.config;
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
                    .creem_checkout_product_for_topup_pack(pack.id)
                    .map(str::to_string)
                else {
                    return ApiResponse::err(
                        "billing_topup_unconfigured",
                        "Creem top-up product is not configured for this pack",
                    );
                };
                match self
                    .creem
                    .client_create_topup_checkout(user_id, pack, &product_id)
                    .await
                {
                    Ok(CheckoutSession::Url { url, session_id }) => {
                        ApiResponse::ok(CheckoutResponse {
                            url,
                            session_id,
                            qr_code: None,
                            order_id: None,
                        })
                    }
                    Ok(CheckoutSession::QrCode { .. }) => ApiResponse::err(
                        "billing_checkout_failed",
                        "Creem top-up checkout returned an unexpected QR session",
                    ),
                    Err(ProviderError::Request(error)) => {
                        ApiResponse::err("billing_checkout_failed", &error)
                    }
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
                // CNY: fen == amount_cents.
                let amount_cents = pack.amount_fen;
                let out_trade_no = uuid::Uuid::new_v4().to_string();

                if let Err(error) = store
                    .insert_pending_alipay_order(
                        user_id,
                        &out_trade_no,
                        pack.id,
                        amount_cents,
                        app_core::PRODUCT_KIND_WALLET_TOPUP,
                    )
                    .await
                {
                    return ApiResponse::err("billing_checkout_failed", &error.to_string());
                }

                let amount_str = app_core::fen_to_decimal_amount(pack.amount_fen);
                let subject = format!("Context OS - Wallet Top-up ¥{}", pack.amount_yuan);
                match self
                    .alipay
                    .client_create_topup_qr(&amount_str, &subject, &out_trade_no)
                    .await
                {
                    Ok(CheckoutSession::QrCode { qr_code, order_id }) => {
                        ApiResponse::ok(CheckoutResponse {
                            url: "".to_string(),
                            session_id: "".to_string(),
                            qr_code: Some(qr_code),
                            order_id: Some(order_id),
                        })
                    }
                    Ok(CheckoutSession::Url { .. }) => ApiResponse::err(
                        "billing_checkout_failed",
                        "Alipay top-up checkout returned an unexpected URL session",
                    ),
                    Err(ProviderError::Request(error)) => {
                        ApiResponse::err("billing_checkout_failed", &error)
                    }
                    Err(error) => ApiResponse::err("billing_checkout_failed", &error.to_string()),
                }
            }
        }
    }

    /// Poll-friendly order status for the Alipay F2F (QR) checkout flow: the
    /// frontend shows the QR code and polls until the webhook marks the order paid.
    ///
    /// If the order is still `pending`, also call `alipay.trade.query` and apply
    /// the same fulfillment path as async notify. This recovers when notify was
    /// missed or failed after signature verification (e.g. lease insert 500).
    pub async fn get_order_status(
        &self,
        store: Arc<dyn BillingStorePort>,
        user_id: UserId,
        order_id: &str,
    ) -> ApiResponse<OrderStatusResponse> {
        let order_id = order_id.trim();
        if order_id.is_empty() {
            return ApiResponse::err("billing_order_invalid", "order id is required");
        }
        match store.load_alipay_order_status(user_id, order_id).await {
            Ok(Some((status, plan_id))) => {
                let mut status = status;
                let mut plan_id = plan_id;
                if status == "pending" && self.config.alipay_enabled() {
                    if let Ok(Some((trade_status, paid_cents))) =
                        self.alipay.client_query_trade(order_id).await
                    {
                        if matches!(
                            trade_status.as_str(),
                            "TRADE_SUCCESS" | "TRADE_FINISHED"
                        ) && paid_cents > 0
                        {
                            let event = app_core::ProviderEvent::AlipayOrderPaid {
                                out_trade_no: order_id.to_string(),
                                paid_cents,
                            };
                            // Idempotent: paid update + subscription upsert / wallet credit.
                            if process_webhook_event(
                                store.clone(),
                                BillingProvider::Alipay,
                                &event,
                            )
                            .await
                            .is_ok()
                            {
                                if let Ok(Some((s, p))) =
                                    store.load_alipay_order_status(user_id, order_id).await
                                {
                                    status = s;
                                    plan_id = p;
                                }
                            }
                            // Query/fulfillment errors stay silent: poll continues on pending.
                        }
                    }
                }
                ApiResponse::ok(OrderStatusResponse {
                    order_id: order_id.to_string(),
                    status,
                    plan_id,
                })
            }
            Ok(None) => ApiResponse::err("billing_order_not_found", "order not found"),
            Err(error) => ApiResponse::err("billing_order_status_failed", &error.to_string()),
        }
    }

    pub async fn handle_webhook(
        &self,
        store: Arc<dyn BillingStorePort>,
        provider: BillingProvider,
        signature: Option<&str>,
        payload: &[u8],
    ) -> ApiResponse<serde_json::Value> {
        // 1. Verify signature and parse into the typed event via the provider
        //    adapter. Stripe provider permanently rejected.
        let delivery = match provider {
            BillingProvider::Stripe => {
                return ApiResponse::err(
                    "billing_provider_removed",
                    "Stripe webhooks are no longer accepted; product billing is Creem + Alipay only",
                );
            }
            BillingProvider::Creem => {
                let adapter = match self.webhook_adapter(provider) {
                    Ok(adapter) => adapter,
                    Err(response) => return response,
                };
                match adapter.parse_event(signature, payload).await {
                    Ok(delivery) => delivery,
                    Err(ProviderError::Signature(message)) => {
                        return ApiResponse::err("billing_webhook_signature_failed", &message);
                    }
                    Err(error) => return webhook_error_response(error.into()),
                }
            }
            BillingProvider::Alipay => {
                let adapter = match self.webhook_adapter(provider) {
                    Ok(adapter) => adapter,
                    Err(response) => return response,
                };
                match adapter.parse_event(signature, payload).await {
                    Ok(delivery) => delivery,
                    Err(ProviderError::Signature(message)) => {
                        return ApiResponse::err("billing_webhook_signature_failed", &message);
                    }
                    Err(error) => return webhook_error_response(error.into()),
                }
            },
        };

        // 2. Lease-based idempotence check on the provider delivery id.
        let claim =
            match claim_webhook_with_lease(store.clone(), provider, &delivery.event_id).await {
                Ok(claim) => claim,
                Err(error) => return webhook_error_response(error),
            };

        if claim.duplicate_processed {
            return ApiResponse::ok(serde_json::json!({
                "status": "ok",
                "duplicate": true,
            }));
        }

        // 3. Process the typed event; `Ignored` acks the delivery with no store
        //    writes (the explicit form of the previous silent no-op).
        if let Err(error) =
            process_webhook_event(store.clone(), provider, &delivery.event).await
        {
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

/// Byte-level percent-decoding so multi-byte UTF-8 values (e.g. Chinese
/// subjects in Alipay notify params) survive signature verification.
pub(crate) fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(hex);
                i += 3;
                continue;
            }
            out.push(b);
            i += 1;
        } else if b == b'+' {
            out.push(b' ');
            i += 1;
        } else {
            out.push(b);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
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
