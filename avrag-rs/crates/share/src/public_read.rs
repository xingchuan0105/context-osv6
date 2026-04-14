use anyhow::{Result, bail};
use avrag_auth::AuthContext;
use chrono::{DateTime, Utc};
use sqlx::Row;
use uuid::Uuid;

use crate::db::{set_current_org, set_current_role, set_public_share_token};
use crate::{
    AccessLevel, PublicShareChatContext, ShareAccessLog, ShareAnalytics, ShareService, SharedKnowledgeBase,
    SharedNotebookPayload, SharedShareInfo, SharedSource,
};

impl ShareService {
    pub async fn get_share_analytics(
        &self,
        ctx: &AuthContext,
        notebook_id: &str,
    ) -> Result<Vec<ShareAnalytics>> {
        if !self
            .check_access(ctx, notebook_id)
            .await?
            .allows_share_management()
        {
            bail!("insufficient permission to view share analytics");
        }
        let notebook_uuid = Uuid::parse_str(notebook_id)?;
        let mut tx = self.repo.raw().begin().await?;
        set_current_org(tx.as_mut(), &ctx.org_id().to_string()).await?;

        // Get token stats aggregated from access logs
        let rows = sqlx::query(
            r#"
            select
                st.token,
                st.access_level,
                count(sal.id) as total_views,
                max(sal.created_at) as last_accessed_at,
                st.created_at
            from share_tokens st
            left join share_access_logs sal on sal.share_token = st.token
            where st.org_id = $1 and st.notebook_id = $2
            group by st.token, st.access_level, st.created_at
            order by total_views desc, max(sal.created_at) desc nulls last
            "#,
        )
        .bind(ctx.org_id().into_uuid())
        .bind(notebook_uuid)
        .fetch_all(tx.as_mut())
        .await?;
        tx.commit().await?;

        Ok(rows
            .into_iter()
            .map(|row| ShareAnalytics {
                token: row.try_get("token").unwrap_or_default(),
                access_level: row.try_get("access_level").unwrap_or_default(),
                total_views: row.try_get::<i64, _>("total_views").unwrap_or_default(),
                last_accessed_at: row
                    .try_get::<Option<DateTime<Utc>>, _>("last_accessed_at")
                    .ok()
                    .flatten()
                    .map(|dt| dt.timestamp()),
                created_at: row
                    .try_get::<Option<DateTime<Utc>>, _>("created_at")
                    .ok()
                    .flatten()
                    .map(|dt| dt.to_rfc3339()),
            })
            .collect())
    }

    pub async fn get_share_access_logs(
        &self,
        ctx: &AuthContext,
        notebook_id: &str,
        limit: usize,
    ) -> Result<Vec<ShareAccessLog>> {
        if !self
            .check_access(ctx, notebook_id)
            .await?
            .allows_share_management()
        {
            bail!("insufficient permission to view access logs");
        }
        let notebook_uuid = Uuid::parse_str(notebook_id)?;
        let mut tx = self.repo.raw().begin().await?;
        set_current_org(tx.as_mut(), &ctx.org_id().to_string()).await?;
        let rows = sqlx::query(
            r#"
            select sal.id, sal.notebook_id, sal.share_token, sal.action, sal.created_at
            from share_access_logs sal
            join share_tokens st on st.token = sal.share_token
            where st.org_id = $1 and st.notebook_id = $2
            order by sal.created_at desc
            limit $3
            "#,
        )
        .bind(ctx.org_id().into_uuid())
        .bind(notebook_uuid)
        .bind(limit as i64)
        .fetch_all(tx.as_mut())
        .await?;
        tx.commit().await?;

        Ok(rows
            .into_iter()
            .map(|row| ShareAccessLog {
                id: row
                    .try_get::<Uuid, _>("id")
                    .map(|u| u.to_string())
                    .unwrap_or_default(),
                notebook_id: row
                    .try_get::<Uuid, _>("notebook_id")
                    .map(|u| u.to_string())
                    .unwrap_or_default(),
                share_token: row.try_get("share_token").unwrap_or_default(),
                action: row.try_get("action").unwrap_or_default(),
                accessed_at: row
                    .try_get::<DateTime<Utc>, _>("created_at")
                    .map(|dt| dt.timestamp())
                    .unwrap_or_default(),
            })
            .collect())
    }

    pub async fn load_shared_notebook(&self, token: &str) -> Result<Option<SharedNotebookPayload>> {
        let mut tx = self.repo.raw().begin().await?;
        set_current_role(tx.as_mut(), "super_admin").await?;
        set_public_share_token(tx.as_mut(), token).await?;
        let row = sqlx::query(
            r#"
            select
              st.org_id,
              st.notebook_id,
              st.access_level,
              st.expires_at,
              n.allow_download
            from share_tokens st
            join notebooks n on n.id = st.notebook_id
            where st.token = $1
              and st.revoked_at is null
              and (st.expires_at is null or st.expires_at > now())
            "#,
        )
        .bind(token)
        .fetch_optional(tx.as_mut())
        .await?;

        let Some(row) = row else {
            tx.rollback().await?;
            return Ok(None);
        };

        let org_id = row.try_get::<Uuid, _>("org_id")?;
        set_current_org(tx.as_mut(), &org_id.to_string()).await?;
        let notebook_id = row.try_get::<Uuid, _>("notebook_id")?;
        let access_level = row.try_get::<String, _>("access_level")?;
        let expires_at = row
            .try_get::<Option<DateTime<Utc>>, _>("expires_at")
            .ok()
            .flatten();
        let allow_download = row.try_get::<bool, _>("allow_download").unwrap_or(false);
        sqlx::query("update share_tokens set access_count = access_count + 1 where token = $1")
            .bind(token)
            .execute(tx.as_mut())
            .await?;
        sqlx::query(
            r#"
            insert into share_access_logs (org_id, notebook_id, share_token, action, created_at)
            values ($1, $2, $3, 'view', now())
            "#,
        )
        .bind(org_id)
        .bind(notebook_id)
        .bind(token)
        .execute(tx.as_mut())
        .await?;
        let notebook_row = sqlx::query("select title, description from notebooks where id = $1")
            .bind(notebook_id)
            .fetch_one(tx.as_mut())
            .await?;
        let title = notebook_row.try_get::<String, _>("title")?;
        let description = notebook_row.try_get::<String, _>("description").ok();

        let sources_rows = sqlx::query(
            r#"
            select id, file_name, status
            from documents
            where notebook_id = $1
            order by updated_at desc, created_at desc
            "#,
        )
        .bind(notebook_id)
        .fetch_all(tx.as_mut())
        .await?;
        tx.commit().await?;

        Ok(Some(SharedNotebookPayload {
            knowledge_base: SharedKnowledgeBase {
                id: notebook_id.to_string(),
                title,
                description,
            },
            share: SharedShareInfo {
                permission: AccessLevel::from_role(&access_level)
                    .as_permission_label()
                    .to_string(),
                expires_at: expires_at.map(|dt| dt.to_rfc3339()),
                allow_download,
                scope: "full".to_string(),
            },
            sources: sources_rows
                .into_iter()
                .map(|row| SharedSource {
                    id: row
                        .try_get::<Uuid, _>("id")
                        .map(|id| id.to_string())
                        .unwrap_or_default(),
                    file_name: row.try_get("file_name").unwrap_or_default(),
                    status: row.try_get("status").unwrap_or_default(),
                })
                .collect(),
        }))
    }

    pub async fn resolve_public_share_chat_context(
        &self,
        token: &str,
    ) -> Result<Option<PublicShareChatContext>> {
        let mut tx = self.repo.raw().begin().await?;
        set_current_role(tx.as_mut(), "super_admin").await?;
        set_public_share_token(tx.as_mut(), token).await?;
        let row = sqlx::query(
            r#"
            select
              st.org_id,
              st.notebook_id,
              st.access_level,
              coalesce(n.owner_id, st.created_by) as owner_user_id
            from share_tokens st
            join notebooks n on n.id = st.notebook_id
            where st.token = $1
              and st.revoked_at is null
              and (st.expires_at is null or st.expires_at > now())
            "#,
        )
        .bind(token)
        .fetch_optional(tx.as_mut())
        .await?;

        let Some(row) = row else {
            tx.rollback().await?;
            return Ok(None);
        };

        let org_id = row.try_get::<Uuid, _>("org_id")?;
        let notebook_id = row.try_get::<Uuid, _>("notebook_id")?;
        let access_level = row.try_get::<String, _>("access_level")?;
        let owner_user_id = row.try_get::<Option<Uuid>, _>("owner_user_id")?;
        let Some(owner_user_id) = owner_user_id else {
            tx.rollback().await?;
            return Ok(None);
        };

        sqlx::query(
            r#"
            insert into share_access_logs (org_id, notebook_id, share_token, action, created_at)
            values ($1, $2, $3, 'chat', now())
            "#,
        )
        .bind(org_id)
        .bind(notebook_id)
        .bind(token)
        .execute(tx.as_mut())
        .await?;
        tx.commit().await?;

        Ok(Some(PublicShareChatContext {
            org_id,
            notebook_id,
            owner_user_id,
            access_level: AccessLevel::from_role(&access_level),
        }))
    }
}
