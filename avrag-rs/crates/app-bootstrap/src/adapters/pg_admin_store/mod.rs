use std::sync::Arc;

use crate::domain_row_convert::{user_profile_row, user_profile_row_to_pg};
use crate::pg_error::map_pg_error;
use app_core::{
    AdminAuditLogEntry, AdminAuditLogPage, AdminAuditLogQuery, AdminBillingOverview,
    AdminDegradationStatus, AdminFeatureFlagChangeRequest, AdminFeatureFlagEntry, AdminAccountInfo,
    AdminRagHealthStatus, AdminStorePort, AdminUsageStats, AdminUserInfo, AdminWorkerStatus,
    admin_audit_logs_to_csv, admin_audit_window_start, admin_clamp_audit_per_page,
    admin_clamp_account_list_per_page, admin_escape_ilike_pattern, admin_usage_period_start,
    domain_rows::UserProfileRow,
};
use async_trait::async_trait;
use contracts::auth_runtime::{AuthContext, UserId};
use avrag_storage_pg::PgAppRepository;
use chrono::{DateTime, Utc};
use common::{ApiKeyRow, AppError, CreateApiKeyResponse, NotificationRow};
use sqlx::{Postgres, QueryBuilder, Row};
use uuid::Uuid;

use crate::adapters::pg_session::{begin_tx, db_err, set_rls_owner, set_current_role};

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
        let actor_id = auth
            .actor_id()
            .ok_or_else(|| AppError::unauthorized("admin action requires an authenticated user"))?;
        let mut tx = begin_tx(self.repo.raw()).await?;
        set_rls_owner(tx.as_mut(), &auth.user_id().to_string()).await?;
        // Personal B2C: role lives on the user row (account == user).
        let role =
            sqlx::query_scalar::<_, String>("select role from users where id = $1")
                .bind(actor_id.into_uuid())
                .fetch_optional(tx.as_mut())
                .await
                .map_err(db_err)?;
        if matches!(
            role.as_deref(),
            Some("super_admin" | "ops_admin" | "finance_admin")
        ) {
            set_current_role(
                tx.as_mut(),
                role.expect("role checked as Some above").as_str(),
            )
            .await?;
            return Ok(tx);
        }
        Err(AppError::forbidden(
            "admin_access_denied",
            "admin access denied",
        ))
    }

    async fn scalar_count(conn: &mut sqlx::PgConnection, sql: &str) -> i64 {
        sqlx::query_scalar::<_, i64>(sql)
            .fetch_one(conn)
            .await
            .unwrap_or(0)
    }
}

include!("mappers.rs");
// Rust 2024 forbids include! inside impl blocks; build.rs assembles shards.lst into OUT_DIR.
include!("port_impl.rs");

#[cfg(test)]
mod tests {
    use crate::adapters::port_shard_guard;
    use app_core::admin_escape_ilike_pattern;

    const ADAPTER_DIR: &str = "src/adapters/pg_admin_store";

    #[test]
    fn all_impl_shards_are_included() {
        port_shard_guard::assert_shards_lst_exists(ADAPTER_DIR);
        let shards = port_shard_guard::parse_shard_list(include_str!("shards.lst"));
        port_shard_guard::assert_shards_exist(ADAPTER_DIR, &shards);
        port_shard_guard::assert_port_impl_includes_out_dir(
            include_str!("port_impl.rs"),
            "pg_admin_store_port_impl.rs",
        );
    }

    #[test]
    fn no_orphan_shard_files() {
        let shards = port_shard_guard::parse_shard_list(include_str!("shards.lst"));
        port_shard_guard::assert_no_orphan_rs_files(ADAPTER_DIR, &shards);
    }

    #[test]
    fn escape_ilike_pattern_treats_percent_as_literal() {
        assert_eq!(admin_escape_ilike_pattern("100%"), r"100\%");
    }

    #[test]
    fn escape_ilike_pattern_treats_underscore_as_literal() {
        assert_eq!(admin_escape_ilike_pattern("a_b"), r"a\_b");
    }
}
