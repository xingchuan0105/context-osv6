use crate::conversation_api::{encode_path_segment, trim_base_url};
use crate::transport::TransportError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BillingPlan {
    pub plan_id: String,
    pub name: String,
    pub description: String,
    pub price_label_cny: String,
    pub interval: String,
    #[serde(default)]
    pub current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BillingPlansResponse {
    pub plans: Vec<BillingPlan>,
    #[serde(default)]
    pub current_plan_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletBalanceResponse {
    pub user_id: String,
    pub balance_fen: i64,
    pub lifetime_paid_topup_fen: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopupPack {
    pub pack_id: String,
    pub amount_fen: i64,
    pub amount_yuan: i64,
    pub label_cny: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckoutRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topup_pack_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckoutResponse {
    pub url: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BillingOrderStatusResponse {
    pub order_id: String,
    pub status: String, // "pending" | "paid"
    pub plan_id: String,
}

pub fn billing_plans_url(base_url: &str) -> String {
    format!("{}/api/v1/billing/plans", trim_base_url(base_url))
}

pub fn wallet_balance_url(base_url: &str) -> String {
    format!("{}/api/v1/billing/wallet", trim_base_url(base_url))
}

pub fn topup_packs_url(base_url: &str) -> String {
    format!("{}/api/v1/billing/wallet/topup-packs", trim_base_url(base_url))
}

pub fn checkout_session_url(base_url: &str) -> String {
    format!("{}/api/v1/billing/checkout-session", trim_base_url(base_url))
}

pub fn order_status_url(base_url: &str, order_id: &str) -> String {
    format!(
        "{}/api/v1/billing/orders/{}",
        trim_base_url(base_url),
        encode_path_segment(order_id)
    )
}

pub fn parse_billing_plans(body: &[u8]) -> Result<BillingPlansResponse, TransportError> {
    serde_json::from_slice(body).map_err(TransportError::from)
}

pub fn parse_wallet_balance(body: &[u8]) -> Result<WalletBalanceResponse, TransportError> {
    serde_json::from_slice(body).map_err(TransportError::from)
}

pub fn parse_topup_packs(body: &[u8]) -> Result<Vec<TopupPack>, TransportError> {
    serde_json::from_slice(body).map_err(TransportError::from)
}

pub fn parse_checkout_response(body: &[u8]) -> Result<CheckoutResponse, TransportError> {
    serde_json::from_slice(body).map_err(TransportError::from)
}

pub fn parse_order_status(body: &[u8]) -> Result<BillingOrderStatusResponse, TransportError> {
    serde_json::from_slice(body).map_err(TransportError::from)
}
