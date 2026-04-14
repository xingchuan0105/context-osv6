use anyhow::{Result, bail};
use avrag_auth::AuthContext;
use chrono::{DateTime, Utc};
use sqlx::Row;
use tracing::warn;
use uuid::Uuid;

use crate::db::{set_current_org, set_current_role, set_public_share_token};
use crate::{
    AccessLevel, ShareService, ShareSettings, ShareTokenInfo,
};

impl ShareService {
    async fn record_share_event(
        &self,
        ctx: &AuthContext,
        notebook_id: &str,
        event_name: analytics::ProductEventName,
        metadata: serde_json::Value,
    ) {
        let Some(user_id) = ctx.actor_id().map(|value| value.into_uuid()) else {
            return;
        };
        let event = analytics::ProductEvent {
            event_id: Uuid::new_v4(),
            event_time: chrono::Utc::now(),
            user_id,
            session_id: None,
            notebook_id: Uuid::parse_str(notebook_id).ok(),
            surface: analytics::Surface::Settings,
            event_name,
            result: analytics::ResultTag::Success,
            request_id: ctx.request_id().map(str::to_string),
            trace_id: None,
            client_platform: "web".to_string(),
            metadata,
        };
        let analytics = analytics::AnalyticsService::new(self.repo.raw().clone());
        if let Err(error) = analytics.record_product_event(&event).await {
            warn!(error = %error, event_name = ?event_name, "failed to record share event");
        }
    }

    pub async fn get_share_settings(
        &self,
        ctx: &AuthContext,
        notebook_id: &str,
    ) -> Result<ShareSettings> {
        let access = self.check_access(ctx, notebook_id).await?;
        if access == AccessLevel::None {
            bail!("insufficient permission to view share settings");
        }
        let notebook_uuid = Uuid::parse_str(notebook_id)?;
        let mut tx = self.repo.raw().begin().await?;
        set_current_org(tx.as_mut(), &ctx.org_id().to_string()).await?;
        let notebook_row =
            sqlx::query("select access_level, allow_download from notebooks where id = $1")
                .bind(notebook_uuid)
                .fetch_one(tx.as_mut())
                .await?;
        let access_level = notebook_row
            .try_get::<String, _>("access_level")
            .unwrap_or_else(|_| "private".to_string());
        let allow_download = notebook_row
            .try_get::<bool, _>("allow_download")
            .unwrap_or(false);
        let share_tokens = sqlx::query(
            r#"
            select token, access_level, expires_at, revoked_at, access_count
            from share_tokens
            where org_id = $1 and notebook_id = $2
            order by created_at desc
            "#,
        )
        .bind(ctx.org_id().into_uuid())
        .bind(notebook_uuid)
        .fetch_all(tx.as_mut())
        .await?;
        tx.commit().await?;

        Ok(ShareSettings {
            access_level,
            allow_download,
            share_tokens: share_tokens
                .into_iter()
                .map(|row| ShareTokenInfo {
                    token: row.try_get("token").unwrap_or_default(),
                    access_level: row.try_get("access_level").unwrap_or_default(),
                    expires_at: row
                        .try_get::<Option<DateTime<Utc>>, _>("expires_at")
                        .ok()
                        .flatten()
                        .map(|value| value.to_rfc3339()),
                    revoked_at: row
                        .try_get::<Option<DateTime<Utc>>, _>("revoked_at")
                        .ok()
                        .flatten()
                        .map(|value| value.to_rfc3339()),
                    access_count: i64::from(
                        row.try_get::<i32, _>("access_count").unwrap_or_default(),
                    ),
                })
                .collect(),
            members: self.list_members(ctx, notebook_id).await?,
        })
    }

    pub async fn update_access_level(
        &self,
        ctx: &AuthContext,
        notebook_id: &str,
        access_level: &str,
    ) -> Result<String> {
        let access = self.check_access(ctx, notebook_id).await?;
        if !access.allows_share_management() {
            bail!("insufficient permission to update access level");
        }
        let normalized = match access_level.trim() {
            "private" | "link" | "public" => access_level.trim(),
            _ => bail!("invalid access level"),
        };
        let mut tx = self.repo.raw().begin().await?;
        set_current_org(tx.as_mut(), &ctx.org_id().to_string()).await?;
        sqlx::query("update notebooks set access_level = $2, updated_at = now() where id = $1")
            .bind(Uuid::parse_str(notebook_id)?)
            .bind(normalized)
            .execute(tx.as_mut())
            .await?;
        tx.commit().await?;
        Ok(normalized.to_string())
    }

    pub async fn update_share_settings(
        &self,
        ctx: &AuthContext,
        notebook_id: &str,
        access_level: Option<&str>,
        allow_download: Option<bool>,
    ) -> Result<ShareSettings> {
        let access = self.check_access(ctx, notebook_id).await?;
        if !access.allows_share_management() {
            bail!("insufficient permission to update share settings");
        }
        let normalized_access_level = access_level
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| match value {
                "private" | "link" | "public" => Ok(value.to_string()),
                _ => bail!("invalid access level"),
            })
            .transpose()?;
        let mut tx = self.repo.raw().begin().await?;
        set_current_org(tx.as_mut(), &ctx.org_id().to_string()).await?;
        sqlx::query(
            r#"
            update notebooks
            set access_level = coalesce($2, access_level),
                allow_download = coalesce($3, allow_download),
                updated_at = now()
            where id = $1
            "#,
        )
        .bind(Uuid::parse_str(notebook_id)?)
        .bind(normalized_access_level.as_deref())
        .bind(allow_download)
        .execute(tx.as_mut())
        .await?;
        tx.commit().await?;
        self.get_share_settings(ctx, notebook_id).await
    }

    pub async fn create_share_token(
        &self,
        ctx: &AuthContext,
        notebook_id: &str,
        access_level: AccessLevel,
        expires_in_secs: Option<i64>,
    ) -> Result<String> {
        if !self
            .check_access(ctx, notebook_id)
            .await?
            .allows_share_management()
        {
            bail!("insufficient permission to create share link");
        }

        let token = Uuid::new_v4().to_string();
        let notebook_uuid = Uuid::parse_str(notebook_id)?;
        let expires_at = expires_in_secs.map(|secs| Utc::now() + chrono::TimeDelta::seconds(secs));
        let mut tx = self.repo.raw().begin().await?;
        set_current_org(tx.as_mut(), &ctx.org_id().to_string()).await?;
        sqlx::query(
            r#"
            insert into share_tokens (token, org_id, notebook_id, access_level, created_by, expires_at)
            values ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(&token)
        .bind(ctx.org_id().into_uuid())
        .bind(notebook_uuid)
        .bind(access_level.as_db())
        .bind(ctx.actor_id().map(|id| id.into_uuid()))
        .bind(expires_at)
        .execute(tx.as_mut())
        .await?;
        tx.commit().await?;
        self.record_share_event(
            ctx,
            notebook_id,
            analytics::ProductEventName::ShareLinkCreated,
            serde_json::json!({
                "access_level": access_level.as_db(),
                "expires_at": expires_at.map(|value| value.to_rfc3339()),
            }),
        )
        .await;
        Ok(token)
    }

    pub async fn validate_token(&self, token: &str) -> Result<Option<(String, AccessLevel)>> {
        let mut tx = self.repo.raw().begin().await?;
        set_current_role(tx.as_mut(), "super_admin").await?;
        set_public_share_token(tx.as_mut(), token).await?;
        let row = sqlx::query(
            r#"
            select notebook_id, access_level
            from share_tokens
            where token = $1
              and revoked_at is null
              and (expires_at is null or expires_at > now())
            "#,
        )
        .bind(token)
        .fetch_optional(tx.as_mut())
        .await?;
        tx.commit().await?;

        Ok(row.map(|row| {
            let notebook_id = row
                .try_get::<Uuid, _>("notebook_id")
                .map(|id| id.to_string())
                .unwrap_or_default();
            let level = row
                .try_get::<String, _>("access_level")
                .map(|role| AccessLevel::from_role(&role))
                .unwrap_or(AccessLevel::None);
            (notebook_id, level)
        }))
    }

    pub async fn revoke_token(&self, ctx: &AuthContext, token: &str) -> Result<()> {
        let mut tx = self.repo.raw().begin().await?;
        set_public_share_token(tx.as_mut(), token).await?;
        let row = sqlx::query(
            "select notebook_id from share_tokens where token = $1 and revoked_at is null",
        )
        .bind(token)
        .fetch_optional(tx.as_mut())
        .await?;
        let Some(row) = row else {
            tx.rollback().await?;
            return Ok(());
        };
        let notebook_id = row.try_get::<Uuid, _>("notebook_id")?.to_string();
        if !self
            .check_access(ctx, &notebook_id)
            .await?
            .allows_share_management()
        {
            tx.rollback().await?;
            bail!("insufficient permission to revoke share link");
        }
        set_current_org(tx.as_mut(), &ctx.org_id().to_string()).await?;
        sqlx::query("update share_tokens set revoked_at = now() where token = $1")
            .bind(token)
            .execute(tx.as_mut())
            .await?;
        tx.commit().await?;
        self.record_share_event(
            ctx,
            &notebook_id,
            analytics::ProductEventName::ShareLinkDisabled,
            serde_json::json!({
                "token": token,
            }),
        )
        .await;
        Ok(())
    }

}
