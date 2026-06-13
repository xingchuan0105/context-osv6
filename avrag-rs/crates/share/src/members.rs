use anyhow::{bail, Result};
use avrag_auth::AuthContext;
use uuid::Uuid;

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
        Ok(self
            .store
            .list_members(ctx, Uuid::parse_str(notebook_id)?)
            .await?
            .into_iter()
            .map(NotebookMember::from)
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
        Ok(self
            .store
            .invite_member(
                ctx,
                Uuid::parse_str(notebook_id)?,
                email,
                access_level.into(),
            )
            .await?
            .into())
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
        self.store
            .accept_invite(
                ctx,
                Uuid::parse_str(notebook_id)?,
                Uuid::parse_str(member_id)?,
                actor_id.into_uuid(),
            )
            .await
            .map_err(map_store_error)
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
        self.store
            .decline_invite(
                ctx,
                Uuid::parse_str(notebook_id)?,
                Uuid::parse_str(member_id)?,
                actor_id.into_uuid(),
            )
            .await
            .map_err(map_store_error)
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
        self.store
            .add_member(
                ctx,
                Uuid::parse_str(notebook_id)?,
                Uuid::parse_str(user_id)?,
                access_level.into(),
            )
            .await
            .map_err(map_store_error)
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
        self.store
            .remove_member(
                ctx,
                Uuid::parse_str(notebook_id)?,
                Uuid::parse_str(member_id)?,
            )
            .await
            .map_err(map_store_error)
    }
}

fn map_store_error(error: common::AppError) -> anyhow::Error {
    anyhow::anyhow!(error.to_string())
}
