//! Payment provider seam — one interface per provider for checkout creation
//! and webhook event parsing.
//!
//! Adapters wrap the low-level HTTP clients ([`crate::CreemClient`],
//! [`crate::AlipayClient`]) and own the provider-specific wire formats:
//! Creem JSON + HMAC-SHA256, Alipay form-urlencoded + RSA2. Product code
//! (`BillingService`, the store trait) only sees [`PaymentProvider`] and the
//! typed [`app_core::ProviderEvent`] vocabulary — no raw provider JSON.

mod alipay;
mod creem;
#[cfg(test)]
mod tests;

pub use alipay::AlipayAdapter;
pub use creem::CreemAdapter;

use app_core::{BillingProvider, ProviderEvent};
use common::UserId;
use thiserror::Error;

/// Errors surfaced by provider adapters. `Signature` and `Invalid` map to the
/// HTTP 400 webhook responses; `Request` to checkout failures.
#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("webhook signature verification failed: {0}")]
    Signature(String),
    #[error("invalid webhook payload: {0}")]
    Invalid(String),
    #[error("provider request failed: {0}")]
    Request(String),
}

/// Provider checkout result. Creem redirects to a hosted URL; Alipay F2F
/// returns a QR code the frontend displays while polling order status.
#[derive(Debug, Clone)]
pub enum CheckoutSession {
    Url {
        url: String,
        session_id: String,
    },
    QrCode {
        qr_code: String,
        order_id: String,
    },
}

/// A verified webhook delivery: the provider's dedupe key plus the typed
/// event. `event_id` is Creem's event `id` / Alipay's `notify_id` — distinct
/// from any domain id inside the event, so the lease dedupes deliveries, not
/// subscriptions.
#[derive(Debug, Clone)]
pub struct ProviderWebhook {
    pub event_id: String,
    pub event: ProviderEvent,
}

/// Provider adapter contract (Creem + Alipay). Implementations must verify
/// signatures before returning any event and must fail explicitly (never
/// silently default) on missing fields.
#[async_trait::async_trait]
pub trait PaymentProvider: Send + Sync {
    fn id(&self) -> BillingProvider;

    /// Create a checkout for `plan_id`. `order_ref` is the provider-specific
    /// order reference (Alipay `out_trade_no`, persisted as a pending order
    /// *before* this call so a webhook cannot race it); Creem ignores it and
    /// generates its own request id.
    async fn create_checkout(
        &self,
        user_id: UserId,
        plan_id: &str,
        order_ref: &str,
    ) -> Result<CheckoutSession, ProviderError>;

    /// Verify `signature` against `raw` and parse into the typed delivery.
    async fn parse_event(
        &self,
        signature: Option<&str>,
        raw: &[u8],
    ) -> Result<ProviderWebhook, ProviderError>;
}
