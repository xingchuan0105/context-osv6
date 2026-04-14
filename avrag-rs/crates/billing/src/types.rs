use chrono::{DateTime, Utc};
use hmac::Hmac;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

pub(crate) const PLAN_FREE: &str = "free";
pub(crate) const PLAN_PRO: &str = "pro";
pub(crate) const PLAN_ENTERPRISE: &str = "enterprise";
pub(crate) const STATUS_ACTIVE: &str = "active";
pub(crate) const STATUS_CANCELED: &str = "canceled";
pub(crate) const STATUS_PAST_DUE: &str = "past_due";
pub(crate) const STATUS_UNPAID: &str = "unpaid";
pub(crate) const ADMIN_ROLE_SUPER: &str = "super_admin";

pub(crate) type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug)]
pub struct BillingConfig {
    pub stripe_secret_key: String,
    pub stripe_webhook_secret: String,
    pub stripe_price_pro: String,
    pub stripe_price_enterprise: String,
    pub billing_price_label_pro: String,
    pub billing_price_label_enterprise: String,
    pub public_app_base_url: String,
}

impl BillingConfig {
    pub fn from_env() -> Self {
        Self {
            stripe_secret_key: std::env::var("STRIPE_SECRET_KEY").unwrap_or_default(),
            stripe_webhook_secret: std::env::var("STRIPE_WEBHOOK_SECRET").unwrap_or_default(),
            stripe_price_pro: std::env::var("STRIPE_PRICE_PRO")
                .or_else(|_| std::env::var("STRIPE_PRICE_PRO_MONTHLY"))
                .or_else(|_| std::env::var("STRIPE_PRICE_ID"))
                .unwrap_or_default(),
            stripe_price_enterprise: std::env::var("STRIPE_PRICE_ENTERPRISE").unwrap_or_default(),
            billing_price_label_pro: std::env::var("BILLING_PRICE_LABEL_PRO")
                .unwrap_or_else(|_| "$20/month".to_string()),
            billing_price_label_enterprise: std::env::var("BILLING_PRICE_LABEL_ENTERPRISE")
                .unwrap_or_else(|_| "Contact sales".to_string()),
            public_app_base_url: std::env::var("PUBLIC_APP_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string()),
        }
    }

    pub fn stripe_enabled(&self) -> bool {
        !self.stripe_secret_key.trim().is_empty()
    }

    pub fn webhook_enabled(&self) -> bool {
        self.stripe_enabled() && !self.stripe_webhook_secret.trim().is_empty()
    }

    pub fn checkout_price_for_plan(&self, plan_id: &str) -> Option<&str> {
        match plan_id.trim() {
            PLAN_PRO if !self.stripe_price_pro.trim().is_empty() => {
                Some(self.stripe_price_pro.as_str())
            }
            PLAN_ENTERPRISE if !self.stripe_price_enterprise.trim().is_empty() => {
                Some(self.stripe_price_enterprise.as_str())
            }
            _ => None,
        }
    }

    pub fn checkout_available(&self, plan_id: &str) -> bool {
        self.stripe_enabled() && self.checkout_price_for_plan(plan_id).is_some()
    }

    pub fn price_label_for_plan(&self, plan_id: &str) -> String {
        match plan_id.trim() {
            PLAN_FREE => "Free".to_string(),
            PLAN_PRO => self.billing_price_label_pro.clone(),
            PLAN_ENTERPRISE => self.billing_price_label_enterprise.clone(),
            _ => String::new(),
        }
    }

    pub fn plan_id_by_price_id(&self, price_id: &str) -> Option<&'static str> {
        let price_id = price_id.trim();
        if price_id.is_empty() {
            return None;
        }
        if price_id == self.stripe_price_pro.trim() {
            return Some(PLAN_PRO);
        }
        if price_id == self.stripe_price_enterprise.trim() {
            return Some(PLAN_ENTERPRISE);
        }
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingPlanQuota {
    pub metric_type: String,
    pub soft_limit: Option<i64>,
    pub hard_limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingPlan {
    pub plan_id: String,
    pub name: String,
    pub description: String,
    pub price_label: String,
    pub interval: String,
    pub checkout_available: bool,
    pub current: bool,
    pub quotas: Vec<BillingPlanQuota>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: String,
    pub org_id: String,
    pub stripe_subscription_id: Option<String>,
    pub stripe_price_id: Option<String>,
    pub plan_id: String,
    pub status: String,
    pub current_period_start: Option<String>,
    pub current_period_end: Option<String>,
    pub cancel_at_period_end: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct StripeSubscriptionSnapshot {
    pub(crate) org_id: String,
    pub(crate) stripe_customer_id: String,
    pub(crate) stripe_subscription_id: String,
    pub(crate) stripe_price_id: String,
    pub(crate) plan_id: String,
    pub(crate) status: String,
    pub(crate) current_period_start: Option<DateTime<Utc>>,
    pub(crate) current_period_end: Option<DateTime<Utc>>,
    pub(crate) cancel_at_period_end: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ExistingSubscriptionFields {
    pub(crate) org_id: String,
    pub(crate) stripe_price_id: String,
    pub(crate) plan_id: String,
}

#[derive(Debug, Clone)]
pub(crate) struct WebhookClaim {
    pub(crate) event_id: String,
    pub(crate) duplicate_processed: bool,
}
