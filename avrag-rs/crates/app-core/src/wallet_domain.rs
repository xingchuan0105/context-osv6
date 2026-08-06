//! User wallet domain types (ADR-0010).
//!
//! **Amount unit: integer fen (分).** 100 fen = ¥1. Signup grant = 2000 fen = ¥20.
//! Do not store yuan floats in the ledger; convert at the API edge only when needed for display.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Signup gift: ¥20 = 2000 fen (分). One-time per user via idempotency key.
pub const SIGNUP_GRANT_FEN: i64 = 2000;

/// Referral bilateral gift: ¥5 = 500 fen each side (ADR-0010 §6).
pub const REFERRAL_BONUS_FEN: i64 = 500;
/// Base rewarded-invite quota before paid top-ups.
pub const REFERRAL_BASE_QUOTA: i64 = 5;
/// Each ¥50 (5000 fen) lifetime paid top-up adds +1 invite quota.
pub const REFERRAL_TOPUP_STEP_FEN: i64 = 5000;

/// Ledger kind: one-time registration gift (ADR-0010 §3.1 / §6).
pub const WALLET_KIND_SIGNUP_GRANT: &str = "signup_grant";
/// Referral bilateral bonus (PR4).
pub const WALLET_KIND_REFERRAL_BONUS: &str = "referral_bonus";
/// Paid wallet top-up via Creem/Alipay (PR5).
pub const WALLET_KIND_TOPUP: &str = "topup";
/// Platform-proxy usage debits (PR6).
pub const WALLET_KIND_USAGE_DEBIT: &str = "usage_debit";
/// Pre-authorize platform spend (negative fen); released on settle/cancel.
pub const WALLET_KIND_USAGE_HOLD: &str = "usage_hold";
/// Release a prior `usage_hold` (positive fen, same hold id).
pub const WALLET_KIND_USAGE_HOLD_RELEASE: &str = "usage_hold_release";

/// Checkout / order product kind: share-slot subscription (existing rails).
pub const PRODUCT_KIND_SUBSCRIPTION: &str = "subscription";
/// Checkout / order product kind: wallet top-up (ADR-0010 PR5).
pub const PRODUCT_KIND_WALLET_TOPUP: &str = "wallet_topup";
/// Checkout request `kind` for subscription (default).
pub const CHECKOUT_KIND_SUBSCRIPTION: &str = "subscription";
/// Checkout request `kind` for wallet top-up.
pub const CHECKOUT_KIND_WALLET_TOPUP: &str = "wallet_topup";

/// Fixed top-up pack ids (fen packs; 100 fen = ¥1).
pub const TOPUP_PACK_50: &str = "topup_50";
pub const TOPUP_PACK_100: &str = "topup_100";
pub const TOPUP_PACK_200: &str = "topup_200";

/// One fixed CNY top-up pack (v1 catalog).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopupPack {
    pub id: &'static str,
    /// Credit amount in fen (分). Also the Alipay `amount_cents` for CNY.
    pub amount_fen: i64,
    /// Display yuan integer (e.g. 50 for ¥50).
    pub amount_yuan: i64,
}

/// Default packs: ¥50 / ¥100 / ¥200.
pub const DEFAULT_TOPUP_PACKS: &[TopupPack] = &[
    TopupPack {
        id: TOPUP_PACK_50,
        amount_fen: 5000,
        amount_yuan: 50,
    },
    TopupPack {
        id: TOPUP_PACK_100,
        amount_fen: 10_000,
        amount_yuan: 100,
    },
    TopupPack {
        id: TOPUP_PACK_200,
        amount_fen: 20_000,
        amount_yuan: 200,
    },
];

/// Resolve a pack by id (`topup_50`, …). Unknown ids return `None`.
pub fn topup_pack_by_id(pack_id: &str) -> Option<&'static TopupPack> {
    let id = pack_id.trim();
    DEFAULT_TOPUP_PACKS.iter().find(|p| p.id == id)
}

/// Alipay `total_amount` / precreate amount string from fen (e.g. 5000 → `"50.00"`).
pub fn fen_to_decimal_amount(fen: i64) -> String {
    let yuan = fen / 100;
    let cents = (fen % 100).abs();
    format!("{yuan}.{cents:02}")
}

/// Stable idempotency key for paid top-up ledger rows.
/// Prefer provider delivery / order id so duplicate webhooks do not double-credit.
pub fn topup_idempotency_key(provider: &str, order_or_event_id: &str) -> String {
    format!("topup:{provider}:{order_or_event_id}")
}

/// Stable idempotency key for the one-time signup grant.
pub fn signup_grant_idempotency_key(user_id: Uuid) -> String {
    format!("signup_grant:{user_id}")
}

/// Inviter-side referral bonus idempotency (keyed by invitee so double path is safe).
pub fn referral_bonus_inviter_idempotency_key(invitee_id: Uuid) -> String {
    format!("referral_bonus:inviter:{invitee_id}")
}

/// Invitee-side referral bonus idempotency.
pub fn referral_bonus_invitee_idempotency_key(invitee_id: Uuid) -> String {
    format!("referral_bonus:invitee:{invitee_id}")
}

/// `referral_quota = 5 + floor(lifetime_paid_topup_fen / 5000)`.
pub fn referral_quota(lifetime_paid_topup_fen: i64) -> i64 {
    let paid = lifetime_paid_topup_fen.max(0);
    REFERRAL_BASE_QUOTA + paid / REFERRAL_TOPUP_STEP_FEN
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Wallet {
    pub user_id: Uuid,
    /// Spendable balance in fen (分).
    pub balance_fen: i64,
    /// Lifetime paid top-ups in fen (excludes gifts).
    pub lifetime_paid_topup_fen: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalletLedgerEntry {
    pub id: Uuid,
    pub user_id: Uuid,
    pub kind: String,
    /// Signed fen delta: credit > 0, debit < 0.
    pub amount_fen: i64,
    pub balance_after_fen: i64,
    pub idempotency_key: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// Result of applying a ledger entry (or replaying an idempotent key).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApplyLedgerResult {
    pub wallet: Wallet,
    /// `true` when a new ledger row was written; `false` when the idempotency key already existed.
    pub applied: bool,
    pub ledger_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct ApplyLedgerInput {
    pub user_id: Uuid,
    pub kind: String,
    /// Signed fen delta: credit > 0, debit < 0.
    pub amount_fen: i64,
    pub idempotency_key: String,
    pub metadata: serde_json::Value,
    /// When true and kind is `topup` with positive amount, also bump `lifetime_paid_topup_fen`.
    pub counts_as_paid_topup: bool,
}
