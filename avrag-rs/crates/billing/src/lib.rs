//! Billing crate — checkout, subscriptions, rolling-window usage quotas, and quota enforcement.
//!
//! Public surface: HTTP handlers in `api`, subscription lifecycle in `core`,
//! rolling limits in `usage_limit`, and unified quota decisions in `quota_service`.

mod alipay_client;
mod api;
mod core;
mod creem_client;
pub mod quota_service;
mod stripe_client;
#[cfg(test)]
mod tests_impl;
mod tier;
mod types;
pub mod usage_limit;

pub use quota_service::{QuotaDenyReason, QuotaManager, UnifiedQuotaDecision};

pub use alipay_client::AlipayClient;
pub use api::{
    CheckoutResponse, CreateCheckoutRequest, PortalResponse, QuotaDecision, SubscriptionResponse,
    UsageResponse, check_quota, handle_create_checkout, handle_create_portal, handle_get_plans,
    handle_get_subscription, handle_get_usage, handle_get_usage_forecast, handle_get_usage_history,
    handle_get_usage_window, handle_webhook,
};
pub use creem_client::CreemClient;
pub use stripe_client::StripeClient;
pub use tier::{BillingTier, ReactLoopAgentMode, ReactLoopBudgetPolicy};
pub use types::{
    BillingConfig, BillingEvent, BillingPlan, BillingPlanQuota, BillingProvider, DailyUsage,
    LimitHits, Subscription, SubscriptionStatus, UsageForecastResponse, UsageHistoryResponse,
    UsageWindowBucket, UsageWindowResponse,
};

pub use core::{expire_subscriptions, process_outbox};
