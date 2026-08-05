//! Postgres wallet + ledger adapter (ADR-0010 PR3).
//!
//! Amounts are integer fen (分). Signup grant = 2000 fen = ¥20.

use std::sync::Arc;

use crate::adapters::pg_session::begin_super_admin_tx_sqlx;
use app_core::{
    ApplyLedgerInput, ApplyLedgerResult, Wallet, WalletLedgerEntry, WalletStorePort,
};
use async_trait::async_trait;
use avrag_storage_pg::PgAppRepository;
use chrono::{DateTime, Utc};
use common::AppError;
use sqlx::Row;
use uuid::Uuid;

pub struct PgWalletStoreAdapter {
    repo: Arc<PgAppRepository>,
}

impl PgWalletStoreAdapter {
    pub fn new(repo: Arc<PgAppRepository>) -> Self {
        Self { repo }
    }
}

fn map_sqlx(error: sqlx::Error) -> AppError {
    AppError::internal(error.to_string())
}

fn wallet_from_row(row: &sqlx::postgres::PgRow) -> Result<Wallet, AppError> {
    Ok(Wallet {
        user_id: row.try_get("user_id").map_err(map_sqlx)?,
        balance_fen: row.try_get("balance_fen").map_err(map_sqlx)?,
        lifetime_paid_topup_fen: row.try_get("lifetime_paid_topup_fen").map_err(map_sqlx)?,
        created_at: row.try_get::<DateTime<Utc>, _>("created_at").map_err(map_sqlx)?,
        updated_at: row.try_get::<DateTime<Utc>, _>("updated_at").map_err(map_sqlx)?,
    })
}

fn ledger_from_row(row: &sqlx::postgres::PgRow) -> Result<WalletLedgerEntry, AppError> {
    Ok(WalletLedgerEntry {
        id: row.try_get("id").map_err(map_sqlx)?,
        user_id: row.try_get("user_id").map_err(map_sqlx)?,
        kind: row.try_get("kind").map_err(map_sqlx)?,
        amount_fen: row.try_get("amount_fen").map_err(map_sqlx)?,
        balance_after_fen: row.try_get("balance_after_fen").map_err(map_sqlx)?,
        idempotency_key: row.try_get("idempotency_key").map_err(map_sqlx)?,
        metadata: row.try_get("metadata").map_err(map_sqlx)?,
        created_at: row.try_get::<DateTime<Utc>, _>("created_at").map_err(map_sqlx)?,
    })
}

#[async_trait]
impl WalletStorePort for PgWalletStoreAdapter {
    async fn get_wallet(&self, user_id: Uuid) -> Result<Option<Wallet>, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;
        let row = sqlx::query(
            "SELECT user_id, balance_fen, lifetime_paid_topup_fen, created_at, updated_at \
             FROM wallets WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        row.map(|r| wallet_from_row(&r)).transpose()
    }

    async fn ensure_wallet(&self, user_id: Uuid) -> Result<Wallet, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;
        sqlx::query(
            "INSERT INTO wallets (user_id) VALUES ($1) ON CONFLICT (user_id) DO NOTHING",
        )
        .bind(user_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_sqlx)?;

        let row = sqlx::query(
            "SELECT user_id, balance_fen, lifetime_paid_topup_fen, created_at, updated_at \
             FROM wallets WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        wallet_from_row(&row)
    }

    async fn apply_ledger_entry(
        &self,
        input: &ApplyLedgerInput,
    ) -> Result<ApplyLedgerResult, AppError> {
        if input.amount_fen == 0 {
            return Err(AppError::validation(
                "wallet_amount_zero",
                "ledger amount_fen must be non-zero",
            ));
        }
        if input.idempotency_key.trim().is_empty() {
            return Err(AppError::validation(
                "wallet_idempotency_required",
                "idempotency_key is required",
            ));
        }

        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;

        // Fast path: existing idempotency key → return current wallet + ledger id.
        if let Some(existing) = sqlx::query(
            "SELECT id, user_id, kind, amount_fen, balance_after_fen, idempotency_key, metadata, created_at \
             FROM wallet_ledger WHERE idempotency_key = $1",
        )
        .bind(&input.idempotency_key)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_sqlx)?
        {
            let entry = ledger_from_row(&existing)?;
            if entry.user_id != input.user_id {
                return Err(AppError::conflict(
                    "wallet_idempotency_user_mismatch",
                    "idempotency_key already used by another user",
                ));
            }
            sqlx::query(
                "INSERT INTO wallets (user_id) VALUES ($1) ON CONFLICT (user_id) DO NOTHING",
            )
            .bind(input.user_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_sqlx)?;
            let wallet_row = sqlx::query(
                "SELECT user_id, balance_fen, lifetime_paid_topup_fen, created_at, updated_at \
                 FROM wallets WHERE user_id = $1",
            )
            .bind(input.user_id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_sqlx)?;
            let wallet = wallet_from_row(&wallet_row)?;
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(ApplyLedgerResult {
                wallet,
                applied: false,
                ledger_id: entry.id,
            });
        }

        // Ensure + lock wallet row.
        sqlx::query(
            "INSERT INTO wallets (user_id) VALUES ($1) ON CONFLICT (user_id) DO NOTHING",
        )
        .bind(input.user_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_sqlx)?;

        let wallet_row = sqlx::query(
            "SELECT user_id, balance_fen, lifetime_paid_topup_fen, created_at, updated_at \
             FROM wallets WHERE user_id = $1 FOR UPDATE",
        )
        .bind(input.user_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_sqlx)?;
        let mut wallet = wallet_from_row(&wallet_row)?;

        let new_balance = wallet.balance_fen + input.amount_fen;
        if new_balance < 0 {
            return Err(AppError::validation(
                "wallet_insufficient_balance",
                "insufficient wallet balance",
            ));
        }

        let ledger_id = Uuid::new_v4();
        let insert = sqlx::query(
            "INSERT INTO wallet_ledger \
                (id, user_id, kind, amount_fen, balance_after_fen, idempotency_key, metadata) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) \
             ON CONFLICT (idempotency_key) DO NOTHING \
             RETURNING id",
        )
        .bind(ledger_id)
        .bind(input.user_id)
        .bind(&input.kind)
        .bind(input.amount_fen)
        .bind(new_balance)
        .bind(&input.idempotency_key)
        .bind(&input.metadata)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_sqlx)?;

        // Concurrent insert won the race — reload as replay.
        if insert.is_none() {
            let existing = sqlx::query(
                "SELECT id, user_id, kind, amount_fen, balance_after_fen, idempotency_key, metadata, created_at \
                 FROM wallet_ledger WHERE idempotency_key = $1",
            )
            .bind(&input.idempotency_key)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_sqlx)?;
            let entry = ledger_from_row(&existing)?;
            let wallet_row = sqlx::query(
                "SELECT user_id, balance_fen, lifetime_paid_topup_fen, created_at, updated_at \
                 FROM wallets WHERE user_id = $1",
            )
            .bind(input.user_id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_sqlx)?;
            let wallet = wallet_from_row(&wallet_row)?;
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(ApplyLedgerResult {
                wallet,
                applied: false,
                ledger_id: entry.id,
            });
        }

        let paid_bump = if input.counts_as_paid_topup && input.amount_fen > 0 {
            input.amount_fen
        } else {
            0
        };

        let updated = sqlx::query(
            "UPDATE wallets \
             SET balance_fen = $2, \
                 lifetime_paid_topup_fen = lifetime_paid_topup_fen + $3, \
                 updated_at = NOW() \
             WHERE user_id = $1 \
             RETURNING user_id, balance_fen, lifetime_paid_topup_fen, created_at, updated_at",
        )
        .bind(input.user_id)
        .bind(new_balance)
        .bind(paid_bump)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_sqlx)?;
        wallet = wallet_from_row(&updated)?;

        tx.commit().await.map_err(map_sqlx)?;
        Ok(ApplyLedgerResult {
            wallet,
            applied: true,
            ledger_id,
        })
    }

    async fn list_ledger(
        &self,
        user_id: Uuid,
        limit: i64,
    ) -> Result<Vec<WalletLedgerEntry>, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;
        let limit = limit.clamp(1, 200);
        let rows = sqlx::query(
            "SELECT id, user_id, kind, amount_fen, balance_after_fen, idempotency_key, metadata, created_at \
             FROM wallet_ledger \
             WHERE user_id = $1 \
             ORDER BY created_at DESC \
             LIMIT $2",
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        rows.iter().map(ledger_from_row).collect()
    }
}
