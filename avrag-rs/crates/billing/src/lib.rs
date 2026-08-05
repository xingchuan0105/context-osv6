//! Billing crate — checkout, subscriptions, rolling-window usage quotas, and quota enforcement.
//!
//! Public surface: `BillingService` and HTTP handlers in `handlers`, subscription
//! lifecycle in `core`, rolling limits in `usage_limit`, and unified quota
//! decisions in `quota_service`.

mod alipay_client;
mod core;
mod creem_client;
mod handlers;
mod payment_provider;
pub mod quota_service;
mod service;
#[cfg(test)]
mod tests_impl;
mod tier;
mod types;
pub mod referral;
pub mod usage_limit;
pub mod wallet;
pub mod wallet_pricing;

pub use quota_service::{QuotaDenyReason, QuotaManager, UnifiedQuotaDecision};

pub use alipay_client::AlipayClient;
pub use creem_client::CreemClient;
pub use service::BillingService;
pub use tier::{BillingTier, ReactLoopAgentMode, ReactLoopBudgetPolicy};
pub use types::{
    BillingConfig, BillingEvent, BillingPlan, BillingPlanQuota, BillingProvider, CreateUsageExportRequest,
    DailyUsage, LimitHits, Subscription, SubscriptionStatus, UsageExportAccepted,
    UsageExportStatusResponse, UsageForecastResponse, UsageHistoryResponse, UsageWindowBucket,
    UsageWindowResponse,
};

pub use handlers::{
    check_quota, handle_create_checkout, handle_create_portal, handle_create_usage_export,
    handle_get_order_status, handle_get_plans, handle_get_subscription, handle_get_usage,
    handle_get_usage_export, handle_get_usage_forecast, handle_get_usage_history,
    handle_get_usage_window, handle_webhook,
};
pub use service::{
    CheckoutResponse, CreateCheckoutRequest, OrderStatusResponse, PortalResponse, QuotaDecision,
    SubscriptionResponse, UsageResponse,
};

pub use core::{expire_subscriptions, process_outbox};

pub use wallet::{
    PaidTopupInput, TopupPackResponse, UsageDebitInput, WalletBalanceResponse, credit_paid_topup,
    debit_platform_usage, get_wallet_balance, grant_signup_bonus, handle_get_wallet,
    handle_list_topup_packs, list_topup_packs,
};
pub use wallet_pricing::{
    LIST_PRICE_MULTIPLIER, OfficialRates, list_price_fen, official_rates_for,
    usage_debit_idempotency_key, usage_debit_idempotency_key_for_request,
};
pub use referral::{
    ApplyReferralOutcome, ReferralStatsResponse, apply_referral_on_register, get_my_referral_stats,
    handle_get_referral,
};
// Re-export fen constants for callers (signup / referral / topup / usage debit, HTTP).
pub use app_core::{
    CHECKOUT_KIND_SUBSCRIPTION, CHECKOUT_KIND_WALLET_TOPUP, DEFAULT_TOPUP_PACKS,
    PRODUCT_KIND_SUBSCRIPTION, PRODUCT_KIND_WALLET_TOPUP, REFERRAL_BASE_QUOTA, REFERRAL_BONUS_FEN,
    REFERRAL_TOPUP_STEP_FEN, SIGNUP_GRANT_FEN, TOPUP_PACK_50, TOPUP_PACK_100, TOPUP_PACK_200,
    TopupPack, WALLET_KIND_REFERRAL_BONUS, WALLET_KIND_SIGNUP_GRANT, WALLET_KIND_TOPUP,
    WALLET_KIND_USAGE_DEBIT, fen_to_decimal_amount, referral_quota, signup_grant_idempotency_key,
    topup_idempotency_key, topup_pack_by_id,
};
