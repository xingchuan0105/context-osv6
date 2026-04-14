use std::sync::Arc;

use anyhow::{bail, Result};
use avrag_auth::{AuthContext, OrgId};
use avrag_storage_pg::PgAppRepository;
use sqlx::Row;

use crate::audit::{
    audit_log_rows, audit_log_total, audit_logs_to_csv, map_audit_log_entry, map_org_info,
    map_user_info, set_current_org, usage_period_start,
};
use crate::models::{AuditLogPage, AuditLogQuery, HealthStatus, OrgInfo, UsageStats, UserInfo};

pub struct AdminService {
    repo: Arc<PgAppRepository>,
}

impl AdminService {
    pub fn new(repo: Arc<PgAppRepository>) -> Self {
        Self { repo }
    }

    async fn ensure_admin(&self, ctx: &AuthContext) -> Result<()> {
        let Some(actor_id) = ctx.actor_id() else {
            bail!("admin access requires an authenticated user");
        };
        let mut tx = self.repo.raw().begin().await?;
        set_current_org(tx.as_mut(), &ctx.org_id().to_string()).await?;
        let row = sqlx::query("select role from users where id = $1 and org_id = $2")
            .bind(actor_id.into_uuid())
            .bind(ctx.org_id().into_uuid())
            .fetch_optional(tx.as_mut())
            .await?;
        let role = row
            .and_then(|row| row.try_get::<String, _>("role").ok())
            .unwrap_or_else(|| "user".to_string());
        if matches!(role.as_str(), "super_admin" | "ops_admin" | "finance_admin") {
            tx.commit().await?;
            return Ok(());
        }
        tx.rollback().await?;
        bail!("admin access denied")
    }

    async fn begin_admin_tx(
        &self,
        ctx: &AuthContext,
    ) -> Result<sqlx::Transaction<'_, sqlx::Postgres>> {
        self.ensure_admin(ctx).await?;
        let Some(actor_id) = ctx.actor_id() else {
            bail!("admin access requires an authenticated user");
        };
        let mut tx = self.repo.raw().begin().await?;
        set_current_org(tx.as_mut(), &ctx.org_id().to_string()).await?;
        let row = sqlx::query("select role from users where id = $1 and org_id = $2")
            .bind(actor_id.into_uuid())
            .bind(ctx.org_id().into_uuid())
            .fetch_optional(tx.as_mut())
            .await?;
        let role = row
            .and_then(|row| row.try_get::<String, _>("role").ok())
            .unwrap_or_else(|| "user".to_string());
        sqlx::query("select set_config('app.current_role', $1, true)")
            .bind(role)
            .execute(tx.as_mut())
            .await?;
        Ok(tx)
    }

    pub async fn list_orgs(&self, ctx: &AuthContext) -> Result<Vec<OrgInfo>> {
        let mut tx = self.begin_admin_tx(ctx).await?;
        let rows = sqlx::query(
            r#"
            select
              o.id,
              o.name,
              o.created_at,
              o.blocked,
              (select count(*) from users u where u.org_id = o.id) as user_count,
              (select count(*) from documents d where d.org_id = o.id) as document_count,
              (select count(*) from chat_messages m where m.org_id = o.id and m.role = 'user') as query_count
            from organizations o
            order by o.created_at desc
            "#,
        )
        .fetch_all(tx.as_mut())
        .await?;
        tx.commit().await?;
        rows.into_iter().map(map_org_info).collect()
    }

    pub async fn get_org(&self, ctx: &AuthContext, org_id: OrgId) -> Result<Option<OrgInfo>> {
        let mut tx = self.begin_admin_tx(ctx).await?;
        let row = sqlx::query(
            r#"
            select
              o.id,
              o.name,
              o.created_at,
              o.blocked,
              (select count(*) from users u where u.org_id = o.id) as user_count,
              (select count(*) from documents d where d.org_id = o.id) as document_count,
              (select count(*) from chat_messages m where m.org_id = o.id and m.role = 'user') as query_count
            from organizations o
            where o.id = $1
            "#,
        )
        .bind(org_id.into_uuid())
        .fetch_optional(tx.as_mut())
        .await?;
        tx.commit().await?;
        row.map(map_org_info).transpose()
    }

    pub async fn list_users(&self, ctx: &AuthContext, org_id: OrgId) -> Result<Vec<UserInfo>> {
        let mut tx = self.begin_admin_tx(ctx).await?;
        let rows = sqlx::query(
            r#"
            select id, email, org_id, role, created_at
            from users
            where org_id = $1
            order by created_at asc
            "#,
        )
        .bind(org_id.into_uuid())
        .fetch_all(tx.as_mut())
        .await?;
        tx.commit().await?;
        rows.into_iter().map(map_user_info).collect()
    }

    pub async fn get_usage(
        &self,
        ctx: &AuthContext,
        org_id: OrgId,
        period: &str,
    ) -> Result<UsageStats> {
        let mut tx = self.begin_admin_tx(ctx).await?;
        let since = usage_period_start(period);
        let row = sqlx::query(
            r#"
            select
              (select count(*) from chat_messages m where m.org_id = $1 and m.role = 'user' and m.created_at >= $2) as query_count,
              (select count(*) from documents d where d.org_id = $1 and d.created_at >= $2) as document_count,
              (select count(*) from chunks c where c.org_id = $1 and c.created_at >= $2) as chunk_count,
              (select coalesce(sum(d.file_size), 0)::bigint from documents d where d.org_id = $1) as storage_bytes
            "#,
        )
        .bind(org_id.into_uuid())
        .bind(since)
        .fetch_one(tx.as_mut())
        .await?;
        tx.commit().await?;

        Ok(UsageStats {
            org_id,
            period: period.to_string(),
            query_count: row.try_get::<i64, _>("query_count")?,
            document_count: row.try_get::<i64, _>("document_count")?,
            chunk_count: row.try_get::<i64, _>("chunk_count")?,
            storage_bytes: row.try_get::<i64, _>("storage_bytes")?,
        })
    }

    pub async fn set_org_blocked(
        &self,
        ctx: &AuthContext,
        org_id: OrgId,
        blocked: bool,
    ) -> Result<()> {
        let mut tx = self.begin_admin_tx(ctx).await?;
        sqlx::query("update organizations set blocked = $2 where id = $1")
            .bind(org_id.into_uuid())
            .bind(blocked)
            .execute(tx.as_mut())
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn list_audit_logs(
        &self,
        ctx: &AuthContext,
        query: &crate::models::AuditLogQuery,
    ) -> Result<AuditLogPage> {
        let mut tx = self.begin_admin_tx(ctx).await?;
        let total = audit_log_total(tx.as_mut(), query).await?;
        let rows = audit_log_rows(tx.as_mut(), query).await?;
        tx.commit().await?;
        Ok(AuditLogPage {
            items: rows
                .into_iter()
                .map(map_audit_log_entry)
                .collect::<Result<_>>()?,
            total,
            page: query.page.max(1),
            per_page: crate::audit::clamp_audit_per_page(query.per_page),
        })
    }

    pub async fn export_audit_logs_csv(
        &self,
        ctx: &AuthContext,
        query: &AuditLogQuery,
    ) -> Result<String> {
        let mut tx = self.begin_admin_tx(ctx).await?;
        let export_query = AuditLogQuery {
            query: query.query.clone(),
            action: query.action.clone(),
            resource_type: query.resource_type.clone(),
            actor: query.actor.clone(),
            window: query.window.clone(),
            page: 1,
            per_page: 5_000,
        };
        let rows = audit_log_rows(tx.as_mut(), &export_query).await?;
        tx.commit().await?;
        Ok(audit_logs_to_csv(
            &rows
                .into_iter()
                .map(map_audit_log_entry)
                .collect::<Result<Vec<_>>>()?,
        ))
    }

    pub async fn get_health() -> HealthStatus {
        HealthStatus {
            status: "ok".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_secs: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
        }
    }
}
