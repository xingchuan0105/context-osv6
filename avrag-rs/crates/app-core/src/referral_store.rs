//! Referral persistence boundary — SQL implementations live in bootstrap adapters.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use common::AppError;
use uuid::Uuid;

use crate::referral_domain::{Referral, ReferralCode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsertPendingResult {
    pub referral: Referral,
    /// `true` when a new pending row was written; `false` when invitee already had a row.
    pub inserted: bool,
}

#[async_trait]
pub trait ReferralStorePort: Send + Sync {
    /// Ensure the user has a stable referral code (create on first need).
    async fn ensure_code(&self, user_id: Uuid) -> Result<ReferralCode, AppError>;

    /// Lookup by normalized code; returns revoked codes too (caller filters).
    async fn find_code(&self, code: &str) -> Result<Option<ReferralCode>, AppError>;

    /// Count only `status = rewarded` for inviter quota.
    async fn count_rewarded_by_inviter(&self, inviter_id: Uuid) -> Result<i64, AppError>;

    /// Count rewarded invites for inviter since `since` (daily antifraud cap).
    async fn count_rewarded_by_inviter_since(
        &self,
        inviter_id: Uuid,
        since: DateTime<Utc>,
    ) -> Result<i64, AppError> {
        let _ = since;
        self.count_rewarded_by_inviter(inviter_id).await
    }

    async fn get_by_invitee(&self, invitee_id: Uuid) -> Result<Option<Referral>, AppError>;

    /// Insert pending if invitee has no row; on unique conflict return existing (`inserted = false`).
    async fn insert_pending(
        &self,
        inviter_id: Uuid,
        invitee_id: Uuid,
        code: &str,
    ) -> Result<InsertPendingResult, AppError>;

    async fn mark_rewarded(&self, referral_id: Uuid) -> Result<Referral, AppError>;

    async fn mark_rejected(
        &self,
        referral_id: Uuid,
        reason: &str,
    ) -> Result<Referral, AppError>;
}
