use anyhow::Result;
use avrag_auth::AuthContext;
use avrag_storage_pg::PgAppRepository;
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

use crate::db::set_current_org;
use crate::{AccessLevel, ShareService};

impl ShareService {
    pub fn new(repo: Arc<PgAppRepository>) -> Self {
        Self { repo }
    }

    pub async fn check_access(&self, ctx: &AuthContext, notebook_id: &str) -> Result<AccessLevel> {
        let notebook_uuid = Uuid::parse_str(notebook_id)?;
        let mut tx = self.repo.raw().begin().await?;
        set_current_org(tx.as_mut(), &ctx.org_id().to_string()).await?;
        let row = sqlx::query(
            r#"
            select owner_id, access_level
            from notebooks
            where id = $1 and org_id = $2
            "#,
        )
        .bind(notebook_uuid)
        .bind(ctx.org_id().into_uuid())
        .fetch_optional(tx.as_mut())
        .await?;
        let Some(row) = row else {
            tx.rollback().await?;
            return Ok(AccessLevel::None);
        };
        let owner_id = row.try_get::<Option<Uuid>, _>("owner_id").ok().flatten();
        let notebook_access_level = row
            .try_get::<String, _>("access_level")
            .unwrap_or_else(|_| "private".to_string());

        if let Some(actor_id) = ctx.actor_id() {
            if owner_id == Some(actor_id.into_uuid()) {
                return Ok(AccessLevel::Admin);
            }
            let row = sqlx::query(
                r#"
                select access_level
                from notebook_members
                where org_id = $1 and notebook_id = $2 and user_id = $3 and invite_status = 'accepted'
                "#,
            )
            .bind(ctx.org_id().into_uuid())
            .bind(notebook_uuid)
            .bind(actor_id.into_uuid())
            .fetch_optional(tx.as_mut())
            .await?;
            if let Some(role) = row.and_then(|row| row.try_get::<String, _>("access_level").ok()) {
                tx.commit().await?;
                return Ok(AccessLevel::from_role(&role));
            }
        }

        if notebook_access_level == "public" {
            tx.commit().await?;
            return Ok(AccessLevel::Read);
        }
        tx.commit().await?;
        Ok(AccessLevel::None)
    }

}
