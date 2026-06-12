use std::sync::Arc;

use async_trait::async_trait;
use app_core::{
    domain_rows::UserProfileRow, AdminBillingOverview, AdminDegradationStatus,
    AdminFeatureFlagChangeRequest, AdminFeatureFlagEntry, AdminRagHealthStatus, AdminStorePort,
    AdminWorkerStatus,
};
use crate::domain_row_convert::{user_profile_row, user_profile_row_to_pg};
use crate::pg_error::map_pg_error;
use avrag_auth::AuthContext;
use avrag_storage_pg::PgAppRepository;
use chrono::{DateTime, Utc};
use common::{ApiKeyRow, AppError, CreateApiKeyResponse, NotificationRow};
use sqlx::Row;
use uuid::Uuid;

pub struct PgAdminStoreAdapter {
    repo: Arc<PgAppRepository>,
}

impl PgAdminStoreAdapter {
    pub fn new(repo: Arc<PgAppRepository>) -> Self {
        Self { repo }
    }

    async fn begin_admin_tx(
        &self,
        auth: &AuthContext,
    ) -> Result<sqlx::Transaction<'_, sqlx::Postgres>, AppError> {
        let actor_id = auth.actor_id().ok_or_else(|| {
            AppError::unauthorized("admin action requires an authenticated user")
        })?;
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        sqlx::query("select set_config('app.current_org', $1, true)")
            .bind(auth.org_id().to_string())
            .execute(tx.as_mut())
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        let role =
            sqlx::query_scalar::<_, String>("select role from users where id = $1 and org_id = $2")
                .bind(actor_id.into_uuid())
                .bind(auth.org_id().into_uuid())
                .fetch_optional(tx.as_mut())
                .await
                .map_err(|error| AppError::internal(error.to_string()))?;
        if matches!(
            role.as_deref(),
            Some("super_admin" | "ops_admin" | "finance_admin")
        ) {
            sqlx::query("select set_config('app.current_role', $1, true)")
                .bind(role.expect("role checked as Some above"))
                .execute(tx.as_mut())
                .await
                .map_err(|error| AppError::internal(error.to_string()))?;
            return Ok(tx);
        }
        Err(AppError::forbidden("admin_access_denied", "admin access denied"))
    }

    async fn scalar_count(conn: &mut sqlx::PgConnection, sql: &str) -> i64 {
        sqlx::query_scalar::<_, i64>(sql)
            .fetch_one(conn)
            .await
            .unwrap_or(0)
    }

    fn map_feature_flag_change_request(row: sqlx::postgres::PgRow) -> AdminFeatureFlagChangeRequest {
        AdminFeatureFlagChangeRequest {
            id: row.try_get("id").unwrap_or_default(),
            flag_key: row.try_get("flag_key").unwrap_or_default(),
            current_enabled: row.try_get("current_enabled").unwrap_or(false),
            requested_enabled: row.try_get("requested_enabled").unwrap_or(false),
            reason: row.try_get("reason").unwrap_or_default(),
            status: row.try_get("status").unwrap_or_default(),
            requested_by: row
                .try_get::<Uuid, _>("requested_by")
                .map(|value| value.to_string())
                .unwrap_or_default(),
            reviewed_by: row
                .try_get::<Option<Uuid>, _>("reviewed_by")
                .ok()
                .flatten()
                .map(|value| value.to_string()),
            review_note: row.try_get("review_note").ok(),
            created_at: row.try_get("created_epoch").unwrap_or(0),
            reviewed_at: row
                .try_get::<Option<i64>, _>("reviewed_epoch")
                .ok()
                .flatten(),
            executed_at: row
                .try_get::<Option<i64>, _>("executed_epoch")
                .ok()
                .flatten(),
        }
    }
}

#[async_trait]
impl AdminStorePort for PgAdminStoreAdapter {
    async fn ensure_admin_access(&self, auth: &AuthContext) -> Result<(), AppError> {
        let tx = self.begin_admin_tx(auth).await?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))
    }

    async fn billing_overview(&self, auth: &AuthContext) -> Result<AdminBillingOverview, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let row = sqlx::query(
            r#"
            select
              count(*) filter (where status = 'active') as active_subscriptions,
              count(*) filter (where status = 'past_due') as past_due_subscriptions,
              count(*) filter (where status = 'unpaid') as unpaid_subscriptions,
              count(*) filter (where status = 'canceled') as canceled_subscriptions
            from subscriptions
            "#,
        )
        .fetch_one(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(AdminBillingOverview {
            active_subscriptions: row.try_get("active_subscriptions").unwrap_or(0),
            past_due_subscriptions: row.try_get("past_due_subscriptions").unwrap_or(0),
            unpaid_subscriptions: row.try_get("unpaid_subscriptions").unwrap_or(0),
            canceled_subscriptions: row.try_get("canceled_subscriptions").unwrap_or(0),
        })
    }

    async fn rag_health(&self, auth: &AuthContext) -> Result<AdminRagHealthStatus, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let response = AdminRagHealthStatus {
            failed_documents: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from documents where status in ('failed','Failed')",
            )
            .await,
            queued_tasks: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from ingestion_tasks where status = 'queued'",
            )
            .await,
            processing_tasks: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from ingestion_tasks where status in ('claimed','processing')",
            )
            .await,
            dead_letter_tasks: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from ingestion_tasks where status = 'dead_letter' or dead_lettered_at is not null",
            )
            .await,
            recent_guard_events: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from audit_log where action like '%guard%' and created_at >= now() - interval '24 hours'",
            )
            .await,
        };
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(response)
    }

    async fn worker_status(&self, auth: &AuthContext) -> Result<AdminWorkerStatus, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let response = AdminWorkerStatus {
            runtime_mode: "milvus",
            queued_tasks: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from ingestion_tasks where status = 'queued'",
            )
            .await,
            processing_tasks: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from ingestion_tasks where status in ('claimed','processing')",
            )
            .await,
            dead_letter_tasks: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from ingestion_tasks where status = 'dead_letter' or dead_lettered_at is not null",
            )
            .await,
            failed_documents: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from documents where status in ('failed','Failed')",
            )
            .await,
        };
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(response)
    }

    async fn degradation_status(
        &self,
        auth: &AuthContext,
    ) -> Result<AdminDegradationStatus, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let response = AdminDegradationStatus {
            failed_documents: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from documents where status in ('failed','Failed')",
            )
            .await,
            recent_guard_events: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from audit_log where action like '%guard%' and created_at >= now() - interval '24 hours'",
            )
            .await,
            share_access_events: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from share_access_logs where created_at >= now() - interval '24 hours'",
            )
            .await,
        };
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(response)
    }

    async fn list_feature_flags(
        &self,
        auth: &AuthContext,
    ) -> Result<Vec<AdminFeatureFlagEntry>, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let rows = sqlx::query(
            r#"
            select f.key, f.enabled, f.source, extract(epoch from f.updated_at)::bigint as updated_at,
              exists(select 1 from feature_flag_change_requests r where r.flag_key = f.key and r.status = 'pending') as has_pending_request
            from feature_flags f
            order by f.key asc
            "#,
        )
        .fetch_all(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|row| AdminFeatureFlagEntry {
                key: row.try_get("key").unwrap_or_default(),
                category: "runtime".to_string(),
                description: String::new(),
                enabled: row.try_get("enabled").unwrap_or(false),
                effective_enabled: row.try_get("enabled").unwrap_or(false),
                config_ready: true,
                requires_config: false,
                source: row
                    .try_get("source")
                    .unwrap_or_else(|_| "admin_panel".to_string()),
                updated_at: row.try_get("updated_at").ok(),
                has_pending_request: row.try_get("has_pending_request").unwrap_or(false),
            })
            .collect())
    }

    async fn list_feature_flag_change_requests(
        &self,
        auth: &AuthContext,
        status: Option<&str>,
    ) -> Result<Vec<AdminFeatureFlagChangeRequest>, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let rows = if let Some(status) = status.filter(|value| !value.trim().is_empty()) {
            sqlx::query("select *, extract(epoch from created_at)::bigint as created_epoch, extract(epoch from reviewed_at)::bigint as reviewed_epoch, extract(epoch from executed_at)::bigint as executed_epoch from feature_flag_change_requests where status = $1 order by created_at desc")
                .bind(status)
                .fetch_all(tx.as_mut())
                .await
        } else {
            sqlx::query("select *, extract(epoch from created_at)::bigint as created_epoch, extract(epoch from reviewed_at)::bigint as reviewed_epoch, extract(epoch from executed_at)::bigint as executed_epoch from feature_flag_change_requests order by created_at desc")
                .fetch_all(tx.as_mut())
                .await
        }
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(rows
            .into_iter()
            .map(Self::map_feature_flag_change_request)
            .collect())
    }

    async fn create_feature_flag_change_request(
        &self,
        auth: &AuthContext,
        key: &str,
        enabled: bool,
        reason: &str,
    ) -> Result<AdminFeatureFlagChangeRequest, AppError> {
        let actor_id = auth.actor_id().ok_or_else(|| {
            AppError::unauthorized("admin action requires an authenticated user")
        })?;
        let mut tx = self.begin_admin_tx(auth).await?;
        sqlx::query(
            "insert into feature_flags (key, enabled) values ($1, false) on conflict (key) do nothing",
        )
        .bind(key)
        .execute(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        let current_enabled =
            sqlx::query_scalar::<_, bool>("select enabled from feature_flags where key = $1")
                .bind(key)
                .fetch_one(tx.as_mut())
                .await
                .unwrap_or(false);
        let id = Uuid::new_v4().to_string();
        let row = sqlx::query("insert into feature_flag_change_requests (id, flag_key, current_enabled, requested_enabled, reason, status, requested_by) values ($1, $2, $3, $4, $5, 'pending', $6) returning *, extract(epoch from created_at)::bigint as created_epoch, extract(epoch from reviewed_at)::bigint as reviewed_epoch, extract(epoch from executed_at)::bigint as executed_epoch")
            .bind(&id)
            .bind(key)
            .bind(current_enabled)
            .bind(enabled)
            .bind(reason)
            .bind(actor_id.into_uuid())
            .fetch_one(tx.as_mut())
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        let response = Self::map_feature_flag_change_request(row);
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(response)
    }

    async fn review_feature_flag_change_request(
        &self,
        auth: &AuthContext,
        request_id: &str,
        approved: bool,
        review_note: Option<&str>,
    ) -> Result<AdminFeatureFlagChangeRequest, AppError> {
        let actor_id = auth.actor_id().ok_or_else(|| {
            AppError::unauthorized("admin action requires an authenticated user")
        })?;
        let mut tx = self.begin_admin_tx(auth).await?;
        let status = if approved { "approved" } else { "rejected" };
        let row = sqlx::query("update feature_flag_change_requests set status = $2, reviewed_by = $3, review_note = $4, reviewed_at = now(), executed_at = case when $2 = 'approved' then now() else executed_at end where id = $1 returning *, extract(epoch from created_at)::bigint as created_epoch, extract(epoch from reviewed_at)::bigint as reviewed_epoch, extract(epoch from executed_at)::bigint as executed_epoch")
            .bind(request_id)
            .bind(status)
            .bind(actor_id.into_uuid())
            .bind(review_note)
            .fetch_one(tx.as_mut())
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        let response = Self::map_feature_flag_change_request(row);
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(response)
    }

    async fn get_user_profile(
        &self,
        auth: &AuthContext,
        user_id: Uuid,
    ) -> Result<Option<UserProfileRow>, AppError> {
        self.repo
            .get_user_profile(auth, user_id)
            .await
            .map_err(map_pg_error)
            .map(|profile| profile.map(user_profile_row))
    }

    async fn upsert_user_profile(
        &self,
        auth: &AuthContext,
        profile: &UserProfileRow,
    ) -> Result<(), AppError> {
        self.repo
            .upsert_user_profile(auth, &user_profile_row_to_pg(profile))
            .await
            .map_err(map_pg_error)
    }

    async fn list_api_keys(
        &self,
        auth: &AuthContext,
        notebook_id: Option<Uuid>,
    ) -> Result<Vec<ApiKeyRow>, AppError> {
        self.repo
            .list_api_keys(auth, notebook_id)
            .await
            .map_err(map_pg_error)
    }

    async fn create_api_key(
        &self,
        auth: &AuthContext,
        notebook_id: Option<Uuid>,
        name: &str,
        permissions: &[String],
        rate_limit_rpm: i32,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<CreateApiKeyResponse, AppError> {
        let (api_key, plaintext_key) = self
            .repo
            .create_api_key(
                auth,
                notebook_id,
                name,
                permissions,
                rate_limit_rpm.max(0) as u32,
                expires_at,
            )
            .await
            .map_err(map_pg_error)?;
        Ok(CreateApiKeyResponse {
            api_key,
            plaintext_key,
        })
    }

    async fn revoke_api_key(
        &self,
        auth: &AuthContext,
        notebook_id: Option<Uuid>,
        key_id: Uuid,
    ) -> Result<bool, AppError> {
        self.repo
            .revoke_api_key(auth, notebook_id, key_id)
            .await
            .map_err(map_pg_error)
    }

    async fn list_notifications(
        &self,
        auth: &AuthContext,
        user_id: Uuid,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<NotificationRow>, AppError> {
        self.repo
            .list_notifications(auth, user_id, limit, offset)
            .await
            .map_err(map_pg_error)
    }

    async fn mark_notification_read(
        &self,
        auth: &AuthContext,
        user_id: Uuid,
        notification_id: Uuid,
    ) -> Result<bool, AppError> {
        self.repo
            .mark_notification_read(auth, user_id, notification_id)
            .await
            .map_err(map_pg_error)
    }
}
