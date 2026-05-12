mod api;
mod core;
mod stripe_client;
#[cfg(test)]
mod tests_impl;
mod types;
pub mod usage_limit;
pub mod quota_service;

pub use quota_service::{QuotaManager, UnifiedQuotaDecision};

pub use api::{
    CheckoutResponse, CreateCheckoutRequest, PortalResponse, QuotaDecision, SubscriptionResponse,
    UsageResponse, check_quota, handle_create_checkout, handle_create_portal, handle_get_plans,
    handle_get_subscription, handle_get_usage, handle_webhook,
};
pub use stripe_client::StripeClient;
pub use types::{BillingConfig, BillingPlan, BillingPlanQuota, Subscription};
