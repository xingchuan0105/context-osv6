use anyhow::{bail, Result};
use avrag_auth::AuthContext;
use chrono::{DateTime, Utc};
use sqlx::Row;
use uuid::Uuid;

use crate::db::set_current_org;
use crate::{AccessLevel, NotebookMember, ShareService};

impl ShareService {
    pub async fn list_members(
        &self,
        ctx: &AuthContext,
        notebook_id: &str,
    ) -> Result<Vec<NotebookMember>> {
        if !self
            .check_access(ctx, notebook_id)
            .await?
            .allows_share_management()
        {
            bail!("insufficient permission to list members");
        }
        let notebook_uuid = Uuid::parse_str(notebook_id)?;
        let mut tx = self.repo.raw().begin().await?;
        set_current_org(tx.as_mut(), &ctx.org_id().to_string()).await?;
        let rows = sqlx::query(
            r#"
            select id, notebook_id, user_id, email, access_level, invite_status, invited_by, invited_at, accepted_at
            from notebook_members
            where org_id = $1 and notebook_id = $2
            order by invited_at asc
            "#,
        )
        .bind(ctx.org_id().into_uuid())
        .bind(notebook_uuid)
        .fetch_all(tx.as_mut())
        .await?;
        tx.commit().await?;

        Ok(rows
            .into_iter()
            .map(|row| NotebookMember {
                id: row
                    .try_get::<Uuid, _>("id")
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
                notebook_id: row
                    .try_get::<Uuid, _>("notebook_id")
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
                user_id: row
                    .try_get::<Option<Uuid>, _>("user_id")
                    .ok()
                    .flatten()
                    .map(|id| id.to_string()),
                email: row.try_get::<Option<String>, _>("email").ok().flatten(),
                access_level: row
                    .try_get::<String, _>("access_level")
                    .map(|role| AccessLevel::from_role(&role))
                    .unwrap_or(AccessLevel::None),
                invite_status: row
                    .try_get::<String, _>("invite_status")
                    .unwrap_or_else(|_| "accepted".to_string()),
                invited_by: row
                    .try_get::<Option<Uuid>, _>("invited_by")
                    .ok()
                    .flatten()
                    .map(|id| id.to_string()),
                invited_at: row
                    .try_get::<DateTime<Utc>, _>("invited_at")
                    .map(|dt| dt.timestamp())
                    .unwrap_or_default(),
                accepted_at: row
                    .try_get::<Option<DateTime<Utc>>, _>("accepted_at")
                    .ok()
                    .flatten()
                    .map(|dt| dt.timestamp()),
            })
            .collect())
    }

    pub async fn invite_member(
        &self,
        ctx: &AuthContext,
        notebook_id: &str,
        email: &str,
        access_level: AccessLevel,
    ) -> Result<NotebookMember> {
        if !self
            .check_access(ctx, notebook_id)
            .await?
            .allows_share_management()
        {
            bail!("insufficient permission to invite members");
        }
        let notebook_uuid = Uuid::parse_str(notebook_id)?;
        let normalized_email = email.trim().to_lowercase();
        if normalized_email.is_empty() {
            bail!("invite email is required");
        }
        let mut tx = self.repo.raw().begin().await?;
        set_current_org(tx.as_mut(), &ctx.org_id().to_string()).await?;
        let invited_user = sqlx::query(
            "select id from users where org_id = $1 and lower(email) = lower($2) limit 1",
        )
        .bind(ctx.org_id().into_uuid())
        .bind(&normalized_email)
        .fetch_optional(tx.as_mut())
        .await?;
        let user_id = invited_user.and_then(|row| row.try_get::<Uuid, _>("id").ok());
        let existing = sqlx::query(
            "select id from notebook_members where org_id = $1 and notebook_id = $2 and lower(email) = lower($3) limit 1",
        )
        .bind(ctx.org_id().into_uuid())
        .bind(notebook_uuid)
        .bind(&normalized_email)
        .fetch_optional(tx.as_mut())
        .await?;
        let row = if let Some(existing) = existing {
            sqlx::query(
                r#"
                update notebook_members
                set user_id = $4,
                    access_level = $5,
                    invited_by = $6,
                    invite_status = 'pending',
                    invited_at = now(),
                    updated_at = now(),
                    accepted_at = null
                where id = $1 and org_id = $2 and notebook_id = $3
                returning id, notebook_id, user_id, email, access_level, invite_status, invited_by, invited_at, accepted_at
                "#,
            )
            .bind(existing.try_get::<Uuid, _>("id")?)
            .bind(ctx.org_id().into_uuid())
            .bind(notebook_uuid)
            .bind(user_id)
            .bind(access_level.as_db())
            .bind(ctx.actor_id().map(|id| id.into_uuid()))
            .fetch_one(tx.as_mut())
            .await?
        } else {
            sqlx::query(
                r#"
                insert into notebook_members (id, org_id, notebook_id, user_id, email, access_level, invited_by, invite_status, invited_at, updated_at)
                values ($1, $2, $3, $4, $5, $6, $7, 'pending', now(), now())
                returning id, notebook_id, user_id, email, access_level, invite_status, invited_by, invited_at, accepted_at
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(ctx.org_id().into_uuid())
            .bind(notebook_uuid)
            .bind(user_id)
            .bind(&normalized_email)
            .bind(access_level.as_db())
            .bind(ctx.actor_id().map(|id| id.into_uuid()))
            .fetch_one(tx.as_mut())
            .await?
        };
        tx.commit().await?;
        Ok(NotebookMember {
            id: row.try_get::<Uuid, _>("id")?.to_string(),
            notebook_id: row.try_get::<Uuid, _>("notebook_id")?.to_string(),
            user_id: row
                .try_get::<Option<Uuid>, _>("user_id")
                .ok()
                .flatten()
                .map(|id| id.to_string()),
            email: row.try_get::<Option<String>, _>("email").ok().flatten(),
            access_level: AccessLevel::from_role(&row.try_get::<String, _>("access_level")?),
            invite_status: row.try_get::<String, _>("invite_status")?,
            invited_by: row
                .try_get::<Option<Uuid>, _>("invited_by")
                .ok()
                .flatten()
                .map(|id| id.to_string()),
            invited_at: row.try_get::<DateTime<Utc>, _>("invited_at")?.timestamp(),
            accepted_at: row
                .try_get::<Option<DateTime<Utc>>, _>("accepted_at")
                .ok()
                .flatten()
                .map(|dt| dt.timestamp()),
        })
    }

    pub async fn accept_invite(
        &self,
        ctx: &AuthContext,
        notebook_id: &str,
        member_id: &str,
    ) -> Result<()> {
        let actor_id = ctx
            .actor_id()
            .ok_or_else(|| anyhow::anyhow!("invite acceptance requires user session"))?;
        let mut tx = self.repo.raw().begin().await?;
        set_current_org(tx.as_mut(), &ctx.org_id().to_string()).await?;
        let actor_email =
            sqlx::query("select lower(email) as email from users where id = $1 and org_id = $2")
                .bind(actor_id.into_uuid())
                .bind(ctx.org_id().into_uuid())
                .fetch_one(tx.as_mut())
                .await?
                .try_get::<String, _>("email")?;
        let row = sqlx::query(
            r#"
            select email, invite_status
            from notebook_members
            where id = $1 and org_id = $2 and notebook_id = $3
            for update
            "#,
        )
        .bind(Uuid::parse_str(member_id)?)
        .bind(ctx.org_id().into_uuid())
        .bind(Uuid::parse_str(notebook_id)?)
        .fetch_optional(tx.as_mut())
        .await?
        .ok_or_else(|| anyhow::anyhow!("invite not found"))?;
        let invite_email = row
            .try_get::<Option<String>, _>("email")
            .ok()
            .flatten()
            .unwrap_or_default()
            .to_lowercase();
        let invite_status = row.try_get::<String, _>("invite_status")?;
        if invite_status != "pending" || (!invite_email.is_empty() && invite_email != actor_email) {
            tx.rollback().await?;
            bail!("invite not allowed");
        }
        sqlx::query(
            r#"
            update notebook_members
            set user_id = $4,
                invite_status = 'accepted',
                accepted_at = now(),
                updated_at = now()
            where id = $1 and org_id = $2 and notebook_id = $3
            "#,
        )
        .bind(Uuid::parse_str(member_id)?)
        .bind(ctx.org_id().into_uuid())
        .bind(Uuid::parse_str(notebook_id)?)
        .bind(actor_id.into_uuid())
        .execute(tx.as_mut())
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn decline_invite(
        &self,
        ctx: &AuthContext,
        notebook_id: &str,
        member_id: &str,
    ) -> Result<()> {
        let actor_id = ctx
            .actor_id()
            .ok_or_else(|| anyhow::anyhow!("invite decline requires user session"))?;
        let mut tx = self.repo.raw().begin().await?;
        set_current_org(tx.as_mut(), &ctx.org_id().to_string()).await?;
        let actor_email =
            sqlx::query("select lower(email) as email from users where id = $1 and org_id = $2")
                .bind(actor_id.into_uuid())
                .bind(ctx.org_id().into_uuid())
                .fetch_one(tx.as_mut())
                .await?
                .try_get::<String, _>("email")?;
        let row = sqlx::query(
            r#"
            select email, invite_status
            from notebook_members
            where id = $1 and org_id = $2 and notebook_id = $3
            for update
            "#,
        )
        .bind(Uuid::parse_str(member_id)?)
        .bind(ctx.org_id().into_uuid())
        .bind(Uuid::parse_str(notebook_id)?)
        .fetch_optional(tx.as_mut())
        .await?
        .ok_or_else(|| anyhow::anyhow!("invite not found"))?;
        let invite_email = row
            .try_get::<Option<String>, _>("email")
            .ok()
            .flatten()
            .unwrap_or_default()
            .to_lowercase();
        let invite_status = row.try_get::<String, _>("invite_status")?;
        if invite_status != "pending" || (!invite_email.is_empty() && invite_email != actor_email) {
            tx.rollback().await?;
            bail!("invite not allowed");
        }
        sqlx::query(
            r#"
            update notebook_members
            set invite_status = 'declined',
                updated_at = now()
            where id = $1 and org_id = $2 and notebook_id = $3
            "#,
        )
        .bind(Uuid::parse_str(member_id)?)
        .bind(ctx.org_id().into_uuid())
        .bind(Uuid::parse_str(notebook_id)?)
        .execute(tx.as_mut())
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn add_member(
        &self,
        ctx: &AuthContext,
        notebook_id: &str,
        user_id: &str,
        access_level: AccessLevel,
    ) -> Result<()> {
        if !self
            .check_access(ctx, notebook_id)
            .await?
            .allows_share_management()
        {
            bail!("insufficient permission to add members");
        }
        let notebook_uuid = Uuid::parse_str(notebook_id)?;
        let user_uuid = Uuid::parse_str(user_id)?;
        let mut tx = self.repo.raw().begin().await?;
        set_current_org(tx.as_mut(), &ctx.org_id().to_string()).await?;
        sqlx::query(
            r#"
            insert into notebook_members (id, org_id, notebook_id, user_id, access_level, invited_by, invite_status, invited_at, accepted_at, updated_at)
            values ($1, $2, $3, $4, $5, $6, 'accepted', now(), now(), now())
            on conflict (notebook_id, user_id) do update
            set access_level = excluded.access_level,
                invited_by = excluded.invited_by,
                invite_status = 'accepted',
                invited_at = now(),
                accepted_at = now(),
                updated_at = now()
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(ctx.org_id().into_uuid())
        .bind(notebook_uuid)
        .bind(user_uuid)
        .bind(access_level.as_db())
        .bind(ctx.actor_id().map(|id| id.into_uuid()))
        .execute(tx.as_mut())
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn remove_member(
        &self,
        ctx: &AuthContext,
        notebook_id: &str,
        member_id: &str,
    ) -> Result<()> {
        if !self
            .check_access(ctx, notebook_id)
            .await?
            .allows_share_management()
        {
            bail!("insufficient permission to remove members");
        }
        let mut tx = self.repo.raw().begin().await?;
        set_current_org(tx.as_mut(), &ctx.org_id().to_string()).await?;
        sqlx::query(
            r#"
            delete from notebook_members
            where org_id = $1 and notebook_id = $2 and id = $3
            "#,
        )
        .bind(ctx.org_id().into_uuid())
        .bind(Uuid::parse_str(notebook_id)?)
        .bind(Uuid::parse_str(member_id)?)
        .execute(tx.as_mut())
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
