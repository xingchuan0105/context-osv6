//! Postgres referral_codes + referrals adapter (ADR-0010 PR4).

use std::sync::Arc;

use crate::adapters::pg_session::begin_super_admin_tx_sqlx;
use app_core::{
    InsertPendingResult, Referral, ReferralCode, ReferralStorePort, generate_referral_code,
    normalize_referral_code, REFERRAL_STATUS_PENDING, REFERRAL_STATUS_REJECTED,
    REFERRAL_STATUS_REWARDED,
};
use async_trait::async_trait;
use avrag_storage_pg::PgAppRepository;
use chrono::{DateTime, Utc};
use common::AppError;
use sqlx::Row;
use uuid::Uuid;

pub struct PgReferralStoreAdapter {
    repo: Arc<PgAppRepository>,
}

impl PgReferralStoreAdapter {
    pub fn new(repo: Arc<PgAppRepository>) -> Self {
        Self { repo }
    }
}

fn map_sqlx(error: sqlx::Error) -> AppError {
    AppError::internal(error.to_string())
}

fn code_from_row(row: &sqlx::postgres::PgRow) -> Result<ReferralCode, AppError> {
    Ok(ReferralCode {
        user_id: row.try_get("user_id").map_err(map_sqlx)?,
        code: row.try_get("code").map_err(map_sqlx)?,
        created_at: row.try_get::<DateTime<Utc>, _>("created_at").map_err(map_sqlx)?,
        revoked_at: row
            .try_get::<Option<DateTime<Utc>>, _>("revoked_at")
            .map_err(map_sqlx)?,
    })
}

fn referral_from_row(row: &sqlx::postgres::PgRow) -> Result<Referral, AppError> {
    Ok(Referral {
        id: row.try_get("id").map_err(map_sqlx)?,
        inviter_id: row.try_get("inviter_id").map_err(map_sqlx)?,
        invitee_id: row.try_get("invitee_id").map_err(map_sqlx)?,
        code: row.try_get("code").map_err(map_sqlx)?,
        status: row.try_get("status").map_err(map_sqlx)?,
        rewarded_at: row
            .try_get::<Option<DateTime<Utc>>, _>("rewarded_at")
            .map_err(map_sqlx)?,
        reject_reason: row.try_get("reject_reason").map_err(map_sqlx)?,
        created_at: row.try_get::<DateTime<Utc>, _>("created_at").map_err(map_sqlx)?,
    })
}

#[async_trait]
impl ReferralStorePort for PgReferralStoreAdapter {
    async fn ensure_code(&self, user_id: Uuid) -> Result<ReferralCode, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;

        if let Some(existing) = sqlx::query(
            "SELECT user_id, code, created_at, revoked_at FROM referral_codes WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_sqlx)?
        {
            tx.commit().await.map_err(map_sqlx)?;
            return code_from_row(&existing);
        }

        // Insert with collision retry on unique code.
        let mut last_err = None;
        for _ in 0..8 {
            let code = generate_referral_code();
            let insert = sqlx::query(
                "INSERT INTO referral_codes (user_id, code) VALUES ($1, $2) \
                 ON CONFLICT (user_id) DO NOTHING \
                 RETURNING user_id, code, created_at, revoked_at",
            )
            .bind(user_id)
            .bind(&code)
            .fetch_optional(tx.as_mut())
            .await;

            match insert {
                Ok(Some(row)) => {
                    tx.commit().await.map_err(map_sqlx)?;
                    return code_from_row(&row);
                }
                Ok(None) => {
                    // Concurrent insert won on user_id — reload.
                    let row = sqlx::query(
                        "SELECT user_id, code, created_at, revoked_at \
                         FROM referral_codes WHERE user_id = $1",
                    )
                    .bind(user_id)
                    .fetch_one(tx.as_mut())
                    .await
                    .map_err(map_sqlx)?;
                    tx.commit().await.map_err(map_sqlx)?;
                    return code_from_row(&row);
                }
                Err(e) => {
                    // Unique code collision — retry with a new code.
                    let msg = e.to_string();
                    if msg.contains("referral_codes_code_key") || msg.contains("duplicate key") {
                        last_err = Some(e);
                        continue;
                    }
                    return Err(map_sqlx(e));
                }
            }
        }
        Err(map_sqlx(last_err.expect("retry loop exhausted without error")))
    }

    async fn find_code(&self, code: &str) -> Result<Option<ReferralCode>, AppError> {
        let normalized = normalize_referral_code(code);
        if normalized.is_empty() {
            return Ok(None);
        }
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;
        let row = sqlx::query(
            "SELECT user_id, code, created_at, revoked_at FROM referral_codes WHERE code = $1",
        )
        .bind(&normalized)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        row.map(|r| code_from_row(&r)).transpose()
    }

    async fn count_rewarded_by_inviter(&self, inviter_id: Uuid) -> Result<i64, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM referrals \
             WHERE inviter_id = $1 AND status = $2",
        )
        .bind(inviter_id)
        .bind(REFERRAL_STATUS_REWARDED)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(count)
    }

    async fn count_rewarded_by_inviter_since(
        &self,
        inviter_id: Uuid,
        since: chrono::DateTime<chrono::Utc>,
    ) -> Result<i64, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM referrals \
             WHERE inviter_id = $1 AND status = $2 \
               AND COALESCE(rewarded_at, created_at) >= $3",
        )
        .bind(inviter_id)
        .bind(REFERRAL_STATUS_REWARDED)
        .bind(since)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(count)
    }

    async fn get_by_invitee(&self, invitee_id: Uuid) -> Result<Option<Referral>, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;
        let row = sqlx::query(
            "SELECT id, inviter_id, invitee_id, code, status, rewarded_at, reject_reason, created_at \
             FROM referrals WHERE invitee_id = $1",
        )
        .bind(invitee_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        row.map(|r| referral_from_row(&r)).transpose()
    }

    async fn insert_pending(
        &self,
        inviter_id: Uuid,
        invitee_id: Uuid,
        code: &str,
    ) -> Result<InsertPendingResult, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;

        let insert = sqlx::query(
            "INSERT INTO referrals (inviter_id, invitee_id, code, status) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (invitee_id) DO NOTHING \
             RETURNING id, inviter_id, invitee_id, code, status, rewarded_at, reject_reason, created_at",
        )
        .bind(inviter_id)
        .bind(invitee_id)
        .bind(code)
        .bind(REFERRAL_STATUS_PENDING)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_sqlx)?;

        if let Some(row) = insert {
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(InsertPendingResult {
                referral: referral_from_row(&row)?,
                inserted: true,
            });
        }

        let existing = sqlx::query(
            "SELECT id, inviter_id, invitee_id, code, status, rewarded_at, reject_reason, created_at \
             FROM referrals WHERE invitee_id = $1",
        )
        .bind(invitee_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(InsertPendingResult {
            referral: referral_from_row(&existing)?,
            inserted: false,
        })
    }

    async fn mark_rewarded(&self, referral_id: Uuid) -> Result<Referral, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;
        let row = sqlx::query(
            "UPDATE referrals \
             SET status = $2, rewarded_at = NOW(), reject_reason = NULL \
             WHERE id = $1 \
             RETURNING id, inviter_id, invitee_id, code, status, rewarded_at, reject_reason, created_at",
        )
        .bind(referral_id)
        .bind(REFERRAL_STATUS_REWARDED)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        let row = row.ok_or_else(|| AppError::not_found("referral_not_found", "referral not found"))?;
        referral_from_row(&row)
    }

    async fn mark_rejected(
        &self,
        referral_id: Uuid,
        reason: &str,
    ) -> Result<Referral, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;
        let row = sqlx::query(
            "UPDATE referrals \
             SET status = $2, reject_reason = $3 \
             WHERE id = $1 \
             RETURNING id, inviter_id, invitee_id, code, status, rewarded_at, reject_reason, created_at",
        )
        .bind(referral_id)
        .bind(REFERRAL_STATUS_REJECTED)
        .bind(reason)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        let row = row.ok_or_else(|| AppError::not_found("referral_not_found", "referral not found"))?;
        referral_from_row(&row)
    }
}
