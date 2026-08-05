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
    WalletBalanceResponse, get_wallet_balance, grant_signup_bonus, handle_get_wallet,
};
pub use referral::{
    ApplyReferralOutcome, ReferralStatsResponse, apply_referral_on_register, get_my_referral_stats,
    handle_get_referral,
};
// Re-export fen constants for callers (signup / referral hook, HTTP).
pub use app_core::{
    REFERRAL_BASE_QUOTA, REFERRAL_BONUS_FEN, REFERRAL_TOPUP_STEP_FEN, SIGNUP_GRANT_FEN,
    WALLET_KIND_REFERRAL_BONUS, WALLET_KIND_SIGNUP_GRANT, referral_quota,
    signup_grant_idempotency_key,
};
