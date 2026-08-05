//! Referral domain types (ADR-0010 §6–7).
//!
//! Bilateral ¥5 (500 fen) on successful register with a valid code, stacked with signup grant.
//! Quota uses inviter `wallets.lifetime_paid_topup_fen` only; does not raise share workspace quota.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Referral row status: only `rewarded` counts against inviter quota.
pub const REFERRAL_STATUS_PENDING: &str = "pending";
pub const REFERRAL_STATUS_REWARDED: &str = "rewarded";
pub const REFERRAL_STATUS_REJECTED: &str = "rejected";

pub const REFERRAL_REJECT_SELF_INVITE: &str = "self_invite";
pub const REFERRAL_REJECT_QUOTA_EXHAUSTED: &str = "quota_exhausted";
pub const REFERRAL_REJECT_CODE_INVALID: &str = "code_invalid";
pub const REFERRAL_REJECT_CODE_REVOKED: &str = "code_revoked";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReferralCode {
    pub user_id: Uuid,
    pub code: String,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Referral {
    pub id: Uuid,
    pub inviter_id: Uuid,
    pub invitee_id: Uuid,
    pub code: String,
    pub status: String,
    pub rewarded_at: Option<DateTime<Utc>>,
    pub reject_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReferralStats {
    pub code: String,
    pub rewarded_count: i64,
    pub quota: i64,
    pub remaining: i64,
    pub lifetime_paid_topup_fen: i64,
}

/// Normalize user-entered code: trim + uppercase (storage uses uppercase COS-XXXXXX).
pub fn normalize_referral_code(raw: &str) -> String {
    raw.trim().to_uppercase()
}

/// Generate a stable short code `COS-` + 6 unambiguous alphanumerics.
pub fn generate_referral_code() -> String {
    // Hex from UUID is fine (0-9A-F); prefix makes product codes recognizable.
    let hex = Uuid::new_v4().simple().to_string().to_uppercase();
    format!("COS-{}", &hex[..6])
}
