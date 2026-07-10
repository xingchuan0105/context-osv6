use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PLAN_FREE: &str = "free";
pub const PLAN_PRO: &str = "pro";
pub const PLAN_PLUS: &str = "plus";
pub const PLAN_DESKTOP_STANDARD: &str = "desktop-standard";
pub const PLAN_DESKTOP_PRO: &str = "desktop-pro";

pub fn is_desktop_license_plan(plan_id: &str) -> bool {
    matches!(
        plan_id.trim(),
        PLAN_DESKTOP_STANDARD | PLAN_DESKTOP_PRO | "standard" | "desktop_pro"
    ) || plan_id.trim().starts_with("desktop-")
}
pub const STATUS_ACTIVE: &str = "active";
pub const STATUS_CANCELED: &str = "canceled";
pub const STATUS_PAST_DUE: &str = "past_due";
pub const STATUS_UNPAID: &str = "unpaid";
pub const ADMIN_ROLE_SUPER: &str = "super_admin";

#[derive(Clone, Debug, Default)]
pub struct BillingConfig {
    pub stripe_secret_key: String,
    pub stripe_webhook_secret: String,
    pub stripe_price_pro: String,
    pub stripe_price_plus: String,
    pub billing_price_label_pro: String,
    pub billing_price_label_plus: String,
    pub public_app_base_url: String,

    // Creem Config
    pub creem_api_key: String,
    pub creem_webhook_secret: String,
    pub creem_price_pro: String,
    pub creem_price_plus: String,
    pub creem_product_pro: String,
    pub creem_product_plus: String,
    pub creem_product_desktop_standard: String,
    pub creem_product_desktop_pro: String,

    // Alipay Config
    pub alipay_app_id: String,
    pub alipay_private_key: String,
    pub alipay_public_key: String,
    pub alipay_gateway_url: String,
    pub alipay_notify_url: Option<String>,
    pub alipay_price_pro: String,
    pub alipay_price_plus: String,
    pub alipay_price_desktop_standard: String,
    pub alipay_price_desktop_pro: String,
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
            stripe_price_plus: std::env::var("STRIPE_PRICE_PLUS")
                .or_else(|_| std::env::var("STRIPE_PRICE_ENTERPRISE"))
                .unwrap_or_default(),
            billing_price_label_pro: std::env::var("BILLING_PRICE_LABEL_PRO")
                .unwrap_or_else(|_| "¥129 / 月 · $19 / 月".to_string()),
            billing_price_label_plus: std::env::var("BILLING_PRICE_LABEL_PLUS")
                .or_else(|_| std::env::var("BILLING_PRICE_LABEL_ENTERPRISE"))
                .unwrap_or_else(|_| "¥49 / 月 · $9 / 月".to_string()),
            public_app_base_url: std::env::var("PUBLIC_APP_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string()),

            // Creem Config
            creem_api_key: std::env::var("CREEM_API_KEY").unwrap_or_default(),
            creem_webhook_secret: std::env::var("CREEM_WEBHOOK_SECRET").unwrap_or_default(),
            creem_product_pro: std::env::var("CREEM_PRODUCT_PRO").unwrap_or_default(),
            creem_product_plus: std::env::var("CREEM_PRODUCT_PLUS").unwrap_or_default(),
            creem_product_desktop_standard: std::env::var("CREEM_PRODUCT_DESKTOP_STANDARD")
                .unwrap_or_default(),
            creem_product_desktop_pro: std::env::var("CREEM_PRODUCT_DESKTOP_PRO")
                .unwrap_or_default(),
            creem_price_pro: std::env::var("CREEM_PRICE_PRO")
                .unwrap_or_else(|_| "5.99".to_string()),
            creem_price_plus: std::env::var("CREEM_PRICE_PLUS")
                .unwrap_or_else(|_| "3.19".to_string()),

            // Alipay Config
            alipay_app_id: std::env::var("ALIPAY_APP_ID").unwrap_or_default(),
            alipay_private_key: std::env::var("ALIPAY_PRIVATE_KEY").unwrap_or_default(),
            alipay_public_key: std::env::var("ALIPAY_PUBLIC_KEY").unwrap_or_default(),
            alipay_gateway_url: std::env::var("ALIPAY_GATEWAY_URL").unwrap_or_else(|_| {
                "https://openapi-sandbox.dl.alipaydev.com/gateway.do".to_string()
            }),
            alipay_notify_url: std::env::var("ALIPAY_NOTIFY_URL")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            alipay_price_pro: std::env::var("ALIPAY_PRICE_PRO")
                .unwrap_or_else(|_| "39.00".to_string()),
            alipay_price_plus: std::env::var("ALIPAY_PRICE_PLUS")
                .unwrap_or_else(|_| "19.00".to_string()),
            alipay_price_desktop_standard: std::env::var("ALIPAY_PRICE_DESKTOP_STANDARD")
                .unwrap_or_else(|_| "299.00".to_string()),
            alipay_price_desktop_pro: std::env::var("ALIPAY_PRICE_DESKTOP_PRO")
                .unwrap_or_else(|_| "699.00".to_string()),
        }
    }

    pub fn stripe_enabled(&self) -> bool {
        !self.stripe_secret_key.trim().is_empty()
    }

    pub fn webhook_enabled(&self) -> bool {
        self.stripe_enabled() && !self.stripe_webhook_secret.trim().is_empty()
    }

    pub fn creem_enabled(&self) -> bool {
        !self.creem_api_key.trim().is_empty()
    }

    pub fn alipay_enabled(&self) -> bool {
        !self.alipay_app_id.trim().is_empty()
    }

    pub fn alipay_price_plus(&self) -> &str {
        &self.alipay_price_plus
    }

    pub fn alipay_price_pro(&self) -> &str {
        &self.alipay_price_pro
    }

    pub fn creem_checkout_price_for_plan(&self, plan_id: &str) -> Option<&str> {
        match plan_id.trim() {
            PLAN_PRO if !self.creem_price_pro.trim().is_empty() => {
                Some(self.creem_price_pro.as_str())
            }
            PLAN_PLUS if !self.creem_price_plus.trim().is_empty() => {
                Some(self.creem_price_plus.as_str())
            }
            _ => None,
        }
    }

    pub fn creem_checkout_product_for_plan(&self, plan_id: &str) -> Option<&str> {
        match plan_id.trim() {
            PLAN_PRO if !self.creem_product_pro.trim().is_empty() => {
                Some(self.creem_product_pro.as_str())
            }
            PLAN_PLUS if !self.creem_product_plus.trim().is_empty() => {
                Some(self.creem_product_plus.as_str())
            }
            PLAN_DESKTOP_STANDARD if !self.creem_product_desktop_standard.trim().is_empty() => {
                Some(self.creem_product_desktop_standard.as_str())
            }
            PLAN_DESKTOP_PRO if !self.creem_product_desktop_pro.trim().is_empty() => {
                Some(self.creem_product_desktop_pro.as_str())
            }
            _ => None,
        }
    }

    pub fn alipay_checkout_price_for_plan(&self, plan_id: &str) -> Option<&str> {
        match plan_id.trim() {
            PLAN_PRO if !self.alipay_price_pro.trim().is_empty() => {
                Some(self.alipay_price_pro.as_str())
            }
            PLAN_PLUS if !self.alipay_price_plus.trim().is_empty() => {
                Some(self.alipay_price_plus.as_str())
            }
            PLAN_DESKTOP_STANDARD if !self.alipay_price_desktop_standard.trim().is_empty() => {
                Some(self.alipay_price_desktop_standard.as_str())
            }
            PLAN_DESKTOP_PRO if !self.alipay_price_desktop_pro.trim().is_empty() => {
                Some(self.alipay_price_desktop_pro.as_str())
            }
            _ => None,
        }
    }

    pub fn checkout_price_for_plan(&self, plan_id: &str) -> Option<&str> {
        match plan_id.trim() {
            PLAN_PRO if !self.stripe_price_pro.trim().is_empty() => {
                Some(self.stripe_price_pro.as_str())
            }
            PLAN_PLUS if !self.stripe_price_plus.trim().is_empty() => {
                Some(self.stripe_price_plus.as_str())
            }
            _ => None,
        }
    }

    pub fn checkout_available(&self, plan_id: &str) -> bool {
        if plan_id.trim() == PLAN_FREE {
            return false;
        }
        (self.creem_enabled() && self.creem_checkout_product_for_plan(plan_id).is_some())
            || (self.alipay_enabled() && self.alipay_checkout_price_for_plan(plan_id).is_some())
    }

    pub fn default_checkout_provider(&self) -> BillingProvider {
        if self.creem_enabled() {
            BillingProvider::Creem
        } else if self.alipay_enabled() {
            BillingProvider::Alipay
        } else {
            BillingProvider::Creem
        }
    }

    pub fn price_label_for_plan(&self, plan_id: &str) -> String {
        match plan_id.trim() {
            PLAN_FREE => "Free".to_string(),
            PLAN_PLUS | PLAN_PRO => format!(
                "{} · {}",
                self.price_label_cny_for_plan(plan_id),
                self.price_label_usd_for_plan(plan_id)
            ),
            _ => String::new(),
        }
    }

    /// CNY price label sourced from `ALIPAY_PRICE_*`.
    pub fn price_label_cny_for_plan(&self, plan_id: &str) -> String {
        match plan_id.trim() {
            PLAN_PLUS => format!("¥{} / 月", self.alipay_price_plus.trim()),
            PLAN_PRO => format!("¥{} / 月", self.alipay_price_pro.trim()),
            _ => String::new(),
        }
    }

    /// USD price label sourced from `CREEM_PRICE_*`.
    pub fn price_label_usd_for_plan(&self, plan_id: &str) -> String {
        match plan_id.trim() {
            PLAN_PLUS => format!("${} / 月", self.creem_price_plus.trim()),
            PLAN_PRO => format!("${} / 月", self.creem_price_pro.trim()),
            _ => String::new(),
        }
    }

    pub fn decimal_price_to_cents(price: &str) -> i64 {
        price
            .trim()
            .parse::<f64>()
            .map(|amount| (amount * 100.0).round() as i64)
            .unwrap_or(0)
    }

    pub fn plan_id_by_price_id(&self, price_id: &str) -> Option<&'static str> {
        let price_id = price_id.trim();
        if price_id.is_empty() {
            return None;
        }
        if price_id == self.stripe_price_pro.trim() {
            return Some(PLAN_PRO);
        }
        if price_id == self.stripe_price_plus.trim() {
            return Some(PLAN_PLUS);
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageWindowBucket {
    pub used: i64,
    pub limit: i64,
    pub percentage: i32,
    pub reset_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LimitHits {
    pub rolling_5h: bool,
    pub rolling_7d: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageWindowResponse {
    pub plan_id: String,
    pub rolling_5h: UsageWindowBucket,
    pub rolling_7d: UsageWindowBucket,
    pub soft_limit_hit: LimitHits,
    pub hard_limit_hit: LimitHits,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DailyUsage {
    pub date: String,
    pub tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageHistoryResponse {
    pub daily: Vec<DailyUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageForecastResponse {
    pub current_plan: String,
    pub avg_30d_tokens: i64,
    pub projected_30d_tokens: i64,
    pub current_limit_7d: i64,
    pub upgrade_recommended: bool,
    pub suggestion_zh: String,
    pub suggestion_en: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingPlan {
    pub plan_id: String,
    pub name: String,
    pub description: String,
    pub price_label: String,
    pub price_label_cny: String,
    pub price_label_usd: String,
    pub interval: String,
    pub checkout_available: bool,
    pub current: bool,
    pub quotas: Vec<BillingPlanQuota>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BillingProvider {
    Stripe,
    Creem,
    Alipay,
}

impl std::fmt::Display for BillingProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stripe => write!(f, "stripe"),
            Self::Creem => write!(f, "creem"),
            Self::Alipay => write!(f, "alipay"),
        }
    }
}

impl std::str::FromStr for BillingProvider {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "stripe" => Ok(Self::Stripe),
            "creem" => Ok(Self::Creem),
            "alipay" => Ok(Self::Alipay),
            other => anyhow::bail!("invalid billing provider: {}", other),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    Active,
    Canceled,
    PastDue,
    Unpaid,
    Expired,
}

impl std::fmt::Display for SubscriptionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Canceled => write!(f, "canceled"),
            Self::PastDue => write!(f, "past_due"),
            Self::Unpaid => write!(f, "unpaid"),
            Self::Expired => write!(f, "expired"),
        }
    }
}

impl std::str::FromStr for SubscriptionStatus {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "canceled" | "cancelled" => Ok(Self::Canceled),
            "past_due" => Ok(Self::PastDue),
            "unpaid" => Ok(Self::Unpaid),
            "expired" => Ok(Self::Expired),
            other => anyhow::bail!("invalid subscription status: {}", other),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: String,
    pub user_id: String,
    pub stripe_subscription_id: Option<String>,
    pub stripe_price_id: Option<String>,
    pub billing_provider: BillingProvider,
    pub provider_subscription_id: Option<String>,
    pub provider_price_id: Option<String>,
    pub plan_id: String,
    pub status: SubscriptionStatus,
    pub current_period_start: Option<DateTime<Utc>>,
    pub current_period_end: Option<DateTime<Utc>>,
    pub cancel_at_period_end: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct StripeSubscriptionSnapshot {
    pub user_id: String,
    pub stripe_customer_id: String,
    pub stripe_subscription_id: String,
    pub stripe_price_id: String,
    pub plan_id: String,
    pub status: String,
    pub current_period_start: Option<DateTime<Utc>>,
    pub current_period_end: Option<DateTime<Utc>>,
    pub cancel_at_period_end: bool,
}

#[derive(Debug, Clone)]
pub struct ExistingSubscriptionFields {
    pub user_id: String,
    pub stripe_price_id: Option<String>,
    pub plan_id: String,
}

#[derive(Debug, Clone)]
pub struct WebhookClaim {
    pub event_id: String,
    pub duplicate_processed: bool,
}

#[derive(Debug, Clone)]
pub enum BillingEvent {
    InvoicePaid {
        new_period_end: Option<DateTime<Utc>>,
    },
    PaymentFailed,
    Cancel,
    Expire,
    Created,
    Updated {
        new_status: SubscriptionStatus,
    },
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum TransitionError {
    #[error("Invalid transition from {from:?} via event {event}: {reason}")]
    InvalidTransition {
        from: SubscriptionStatus,
        event: String,
        reason: String,
    },
}

impl Subscription {
    pub fn apply_transition(
        &self,
        event: &BillingEvent,
    ) -> Result<SubscriptionStatus, TransitionError> {
        match event {
            BillingEvent::InvoicePaid { new_period_end } => {
                match self.status {
                    SubscriptionStatus::Active
                    | SubscriptionStatus::PastDue
                    | SubscriptionStatus::Expired
                    | SubscriptionStatus::Canceled
                    | SubscriptionStatus::Unpaid => {
                        match (new_period_end, self.current_period_end) {
                            (Some(new_dt), Some(old_dt)) => {
                                if new_dt < &old_dt {
                                    return Err(TransitionError::InvalidTransition {
                                        from: self.status,
                                        event: "InvoicePaid".to_string(),
                                        reason: "new_period_end cannot be earlier than current_period_end".to_string(),
                                    });
                                }
                            }
                            (None, Some(_)) => {
                                return Err(TransitionError::InvalidTransition {
                                    from: self.status,
                                    event: "InvoicePaid".to_string(),
                                    reason: "new_period_end cannot be None when current_period_end is set".to_string(),
                                });
                            }
                            _ => {}
                        }
                        Ok(SubscriptionStatus::Active)
                    }
                }
            }
            BillingEvent::PaymentFailed => match self.status {
                SubscriptionStatus::Active => Ok(SubscriptionStatus::PastDue),
                SubscriptionStatus::PastDue => Ok(SubscriptionStatus::Unpaid),
                _ => Ok(self.status),
            },
            BillingEvent::Cancel => match self.status {
                SubscriptionStatus::Active
                | SubscriptionStatus::PastDue
                | SubscriptionStatus::Unpaid => Ok(SubscriptionStatus::Canceled),
                _ => Ok(self.status),
            },
            BillingEvent::Expire => Ok(SubscriptionStatus::Expired),
            BillingEvent::Created => Ok(SubscriptionStatus::Active),
            BillingEvent::Updated { new_status } => Ok(*new_status),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillableFeature {
    Summary,
    Planner,
    Answer,
    Search,
    Chat,
    GraphExtraction,
}

impl BillableFeature {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Planner => "planner",
            Self::Answer => "answer",
            Self::Search => "search",
            Self::Chat => "chat",
            Self::GraphExtraction => "graph_extraction",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteringContext {
    pub user_id: Uuid,
    pub owner_user_id: Uuid,
    pub feature: BillableFeature,
    pub stage: String,
    pub session_id: Option<Uuid>,
    pub document_id: Option<Uuid>,
    pub request_id: Option<String>,
    pub trace_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageSource {
    Actual,
    Estimated,
}

impl UsageSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Actual => "actual",
            Self::Estimated => "estimated",
        }
    }
}
