use anyhow::Result;
use contracts::auth_runtime::AuthContext;
use uuid::Uuid;

use crate::{AccessLevel, ShareService};

impl ShareService {
    pub async fn check_access(&self, ctx: &AuthContext, notebook_id: &str) -> Result<AccessLevel> {
        let notebook_uuid = Uuid::parse_str(notebook_id)?;
        let Some(snapshot) = self.store.query_notebook_access(ctx, notebook_uuid).await? else {
            return Ok(AccessLevel::None);
        };

        if let Some(actor_id) = ctx.actor_id() {
            if snapshot.owner_id == Some(actor_id.into_uuid()) {
                return Ok(AccessLevel::Admin);
            }
            if let Some(role) = self
                .store
                .query_member_access(ctx, notebook_uuid, actor_id.into_uuid())
                .await?
            {
                return Ok(AccessLevel::from_role(&role));
            }
        }

        if snapshot.notebook_access_level == "public" {
            return Ok(AccessLevel::Read);
        }
        Ok(AccessLevel::None)
    }
}
