use std::sync::Arc;

use crate::adapters::pg_session::begin_super_admin_tx_sqlx;
use crate::pg_error::map_pg_error;
use app_core::{
    AuthStorePort, AuthUserCredentials, AuthUserProfile, CreatePasswordResetTicketInput,
    PasswordResetUser, RecordLegalAcceptanceInput, RegisterUserInput, RegisterUserResult,
    UserLegalStatus,
};
use async_trait::async_trait;
use avrag_storage_pg::PgAppRepository;
use chrono::{DateTime, Utc};
use common::AppError;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub struct PgAuthStoreAdapter {
    repo: Arc<PgAppRepository>,
}

impl PgAuthStoreAdapter {
    pub fn new(repo: Arc<PgAppRepository>) -> Self {
        Self { repo }
    }
}

fn hash_reset_value(secret: &str, scope: &str, value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.update(b":");
    hasher.update(scope.as_bytes());
    hasher.update(b":");
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())
}

#[async_trait]
impl AuthStorePort for PgAuthStoreAdapter {
    async fn register_user(
        &self,
        input: &RegisterUserInput,
    ) -> Result<RegisterUserResult, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool)
            .await
            .map_err(map_sqlx_error)?;

        match sqlx::query("SELECT id FROM users WHERE email = $1")
            .bind(input.email.trim())
            .fetch_optional(tx.as_mut())
            .await
        {
            Ok(Some(_)) => {
                return Err(AppError::conflict(
                    "email_exists",
                    "An account with this email already exists",
                ));
            }
            Err(error) => return Err(map_sqlx_error(error)),
            _ => {}
        }

        // Personal B2C: one user row is the account; no organizations table.
        let user_id = Uuid::new_v4();
        let full_name = input.full_name.as_deref().unwrap_or_default();

        if let Err(error) = sqlx::query(
            "INSERT INTO users (id, email, full_name, password_hash, role) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(user_id)
        .bind(input.email.trim())
        .bind(full_name)
        .bind(&input.password_hash)
        .bind(contracts::USER_ROLE_ORG_ADMIN)
        .execute(tx.as_mut())
        .await
        {
            return Err(map_sqlx_error(error));
        }

        if let Err(error) = sqlx::query(
                "INSERT INTO legal_acceptances (user_id, terms_version, privacy_version, context, ip_address, user_agent)
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(user_id)
            .bind(&input.legal_acceptance.terms_version)
            .bind(&input.legal_acceptance.privacy_version)
            .bind(&input.legal_acceptance.context)
            .bind(&input.legal_acceptance.ip_address)
            .bind(&input.legal_acceptance.user_agent)
            .execute(tx.as_mut())
            .await
        {
            return Err(map_sqlx_error(error));
        }

        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(RegisterUserResult {
            user_id,
            // Personal account: owner == user (legacy field name kept for JWT issuance).
            owner_user_id: user_id,
            email: input.email.trim().to_string(),
            full_name: full_name.to_string(),
            auth_version: 1,
            role: contracts::USER_ROLE_ORG_ADMIN.to_string(),
        })
    }

    async fn record_legal_acceptance(
        &self,
        input: &RecordLegalAcceptanceInput,
    ) -> Result<(), AppError> {
        let pool = self.repo.raw();
        sqlx::query(
            "INSERT INTO legal_acceptances (user_id, terms_version, privacy_version, context, ip_address, user_agent)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(input.user_id)
        .bind(&input.terms_version)
        .bind(&input.privacy_version)
        .bind(&input.context)
        .bind(&input.ip_address)
        .bind(&input.user_agent)
        .execute(pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn get_user_legal_status(&self, user_id: Uuid) -> Result<UserLegalStatus, AppError> {
        let pool = self.repo.raw();
        let row = sqlx::query_as::<_, (String, String)>(
            "SELECT terms_version, privacy_version
             FROM legal_acceptances
             WHERE user_id = $1
             ORDER BY accepted_at DESC
             LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(map_sqlx_error)?;

        let (accepted_terms_version, accepted_privacy_version) = row
            .map(|(terms, privacy)| (Some(terms), Some(privacy)))
            .unwrap_or((None, None));

        let published_terms_version = app_core::PUBLISHED_TERMS_VERSION.to_string();
        let published_privacy_version = app_core::PUBLISHED_PRIVACY_VERSION.to_string();
        let needs_re_acceptance = match (&accepted_terms_version, &accepted_privacy_version) {
            (Some(terms), Some(privacy)) => {
                terms != &published_terms_version || privacy != &published_privacy_version
            }
            _ => true,
        };

        Ok(UserLegalStatus {
            needs_re_acceptance,
            accepted_terms_version,
            accepted_privacy_version,
            published_terms_version,
            published_privacy_version,
        })
    }

    async fn has_payment_legal_acceptance(&self, user_id: Uuid) -> Result<bool, AppError> {
        let pool = self.repo.raw();
        let published_terms_version = app_core::PUBLISHED_TERMS_VERSION;
        let published_privacy_version = app_core::PUBLISHED_PRIVACY_VERSION;
        let exists = sqlx::query_scalar::<_, i32>(
            "SELECT 1
             FROM legal_acceptances
             WHERE user_id = $1
               AND context = 'payment'
               AND terms_version = $2
               AND privacy_version = $3
             LIMIT 1",
        )
        .bind(user_id)
        .bind(published_terms_version)
        .bind(published_privacy_version)
        .fetch_optional(pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(exists.is_some())
    }

    async fn find_user_for_login(
        &self,
        email: &str,
    ) -> Result<Option<AuthUserCredentials>, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool)
            .await
            .map_err(map_sqlx_error)?;
        // B2C: users row is the account; personal owner == user id.
        let row = sqlx::query_as::<_, (Uuid, String, Option<String>, Option<String>, i32, String)>(
            "SELECT id, email, full_name, password_hash, auth_version, role FROM users WHERE email = $1",
        )
        .bind(email.trim())
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_sqlx_error)?;
        tx.commit().await.map_err(map_sqlx_error)?;

        Ok(row.map(
            |(user_id, email, full_name, password_hash, auth_version, role)| {
                AuthUserCredentials {
                    user_id,
                    owner_user_id: user_id,
                    email,
                    full_name,
                    password_hash,
                    auth_version,
                    role,
                }
            },
        ))
    }

    async fn invalidate_session(&self, user_id: Uuid) -> Result<bool, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool)
            .await
            .map_err(map_sqlx_error)?;
        let result = sqlx::query(
            r#"
            update users
            set auth_version = auth_version + 1
            where id = $1
            "#,
        )
        .bind(user_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_sqlx_error)?;
        if result.rows_affected() == 0 {
            return Ok(false);
        }
        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(true)
    }

    async fn get_user_profile(&self, user_id: Uuid) -> Result<Option<AuthUserProfile>, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool)
            .await
            .map_err(map_sqlx_error)?;
        let row = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
            "SELECT id, email, full_name FROM users WHERE id = $1",
        )
        .bind(user_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_sqlx_error)?;
        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(row.map(|(user_id, email, full_name)| AuthUserProfile {
            user_id,
            owner_user_id: user_id,
            email,
            full_name,
        }))
    }

    async fn update_user_profile(
        &self,
        user_id: Uuid,
        full_name: &str,
    ) -> Result<Option<AuthUserProfile>, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool)
            .await
            .map_err(map_sqlx_error)?;
        let row = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
            r#"
            update users
            set full_name = $2
            where id = $1
            returning id, email, full_name
            "#,
        )
        .bind(user_id)
        .bind(full_name)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_sqlx_error)?;
        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(row.map(|(user_id, email, full_name)| AuthUserProfile {
            user_id,
            owner_user_id: user_id,
            email,
            full_name,
        }))
    }

    async fn get_password_hash(&self, user_id: Uuid) -> Result<Option<String>, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool)
            .await
            .map_err(map_sqlx_error)?;
        let row = sqlx::query_as::<_, (String,)>("SELECT password_hash FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_sqlx_error)?;
        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(row.map(|(password_hash,)| password_hash))
    }

    async fn change_password(&self, user_id: Uuid, password_hash: &str) -> Result<(), AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool)
            .await
            .map_err(map_sqlx_error)?;
        sqlx::query(
            r#"
            update users
            set password_hash = $2,
                password_updated_at = now(),
                auth_version = auth_version + 1
            where id = $1
            "#,
        )
        .bind(user_id)
        .bind(password_hash)
        .execute(tx.as_mut())
        .await
        .map_err(map_sqlx_error)?;
        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn find_user_by_email_for_reset(
        &self,
        email: &str,
    ) -> Result<Option<PasswordResetUser>, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool)
            .await
            .map_err(map_sqlx_error)?;
        let row = sqlx::query_as::<_, (Uuid, String)>(
            r#"
            select id, email
            from users
            where lower(email) = lower($1)
            order by created_at desc
            limit 1
            "#,
        )
        .bind(email)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_sqlx_error)?;
        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(row.map(|(user_id, email)| PasswordResetUser {
            user_id,
            owner_user_id: user_id,
            email,
        }))
    }

    async fn create_password_reset_ticket(
        &self,
        input: &CreatePasswordResetTicketInput,
    ) -> Result<(), AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool)
            .await
            .map_err(map_sqlx_error)?;
        sqlx::query(
            r#"
            insert into password_reset_tickets (
                owner_user_id, user_id, email, purpose, ticket_hash, code_hash,
                expires_at, code_expires_at, attempts, used_at, created_at, updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, 0, null, now(), now())
            "#,
        )
        .bind(input.owner_user_id)
        .bind(input.user_id)
        .bind(&input.email)
        .bind(&input.purpose)
        .bind(&input.ticket_hash)
        .bind(&input.code_hash)
        .bind(input.expires_at)
        .bind(input.code_expires_at)
        .execute(tx.as_mut())
        .await
        .map_err(map_sqlx_error)?;
        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn verify_reset_ticket_exists(&self, ticket_hash: &str) -> Result<bool, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool)
            .await
            .map_err(map_sqlx_error)?;
        let row = sqlx::query(
            r#"
            select 1
            from password_reset_tickets
            where ticket_hash = $1
              and used_at is null
              and expires_at > now()
            limit 1
            "#,
        )
        .bind(ticket_hash)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_sqlx_error)?;
        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(row.is_some())
    }

    async fn verify_and_rotate_reset_code(
        &self,
        email: &str,
        purpose: &str,
        code: &str,
        reset_code_secret: &str,
        new_ticket_hash: &str,
        max_attempts: i32,
    ) -> Result<Option<(Uuid, String)>, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool)
            .await
            .map_err(map_sqlx_error)?;
        let row = sqlx::query_as::<
            _,
            (
                Uuid,
                Uuid,
                String,
                Option<String>,
                i32,
                Option<DateTime<Utc>>,
            ),
        >(
            r#"
            select id, user_id, email, code_hash, attempts, code_expires_at
            from password_reset_tickets
            where lower(email) = lower($1)
              and purpose = $2
              and used_at is null
            order by created_at desc
            limit 1
            for update
            "#,
        )
        .bind(email)
        .bind(purpose)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_sqlx_error)?;

        let Some((ticket_id, user_id, resolved_email, code_hash, attempts, code_expires_at)) = row
        else {
            tx.commit().await.map_err(map_sqlx_error)?;
            return Ok(None);
        };

        if attempts >= max_attempts
            || code_expires_at
                .map(|value| value < chrono::Utc::now())
                .unwrap_or(true)
        {
            tx.commit().await.map_err(map_sqlx_error)?;
            return Ok(None);
        }

        let expected_code_hash = hash_reset_value(
            reset_code_secret,
            purpose,
            &format!("{resolved_email}:{code}"),
        );
        if code_hash.as_deref() != Some(expected_code_hash.as_str()) {
            sqlx::query(
                "update password_reset_tickets set attempts = attempts + 1, updated_at = now() where id = $1",
            )
            .bind(ticket_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_sqlx_error)?;
            tx.commit().await.map_err(map_sqlx_error)?;
            return Ok(None);
        }

        sqlx::query(
            r#"
            update password_reset_tickets
            set ticket_hash = $2,
                updated_at = now()
            where id = $1
            "#,
        )
        .bind(ticket_id)
        .bind(new_ticket_hash)
        .execute(tx.as_mut())
        .await
        .map_err(map_sqlx_error)?;
        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(Some((user_id, resolved_email)))
    }

    async fn reset_password_with_ticket_hash(
        &self,
        ticket_hash: &str,
        purpose: &str,
        password_hash: &str,
    ) -> Result<Uuid, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool)
            .await
            .map_err(map_sqlx_error)?;
        let row = sqlx::query_as::<_, (Uuid, Uuid)>(
            r#"
            select id, user_id
            from password_reset_tickets
            where ticket_hash = $1
              and purpose = $2
              and used_at is null
              and expires_at > now()
            limit 1
            for update
            "#,
        )
        .bind(ticket_hash)
        .bind(purpose)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_sqlx_error)?;

        let Some((ticket_id, user_id)) = row else {
            return Err(AppError::validation(
                "invalid_reset_ticket",
                "Reset session is invalid or expired",
            ));
        };

        sqlx::query(
            r#"
            update users
            set password_hash = $2,
                password_updated_at = now(),
                auth_version = auth_version + 1
            where id = $1
            "#,
        )
        .bind(user_id)
        .bind(password_hash)
        .execute(tx.as_mut())
        .await
        .map_err(map_sqlx_error)?;
        sqlx::query(
            "update password_reset_tickets set used_at = now(), updated_at = now() where id = $1",
        )
        .bind(ticket_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_sqlx_error)?;
        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(user_id)
    }
}

fn map_sqlx_error(error: sqlx::Error) -> AppError {
    map_pg_error(error.into())
}
