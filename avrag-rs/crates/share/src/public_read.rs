use anyhow::{bail, Result};

use crate::{
    PublicShareChatContext, ShareAccessLog, ShareService, SharedKnowledgeBase,
    SharedWorkspacePayload, SharedShareInfo, SharedSource,
};

impl ShareService {
    pub async fn get_share_access_logs(
        &self,
        ctx: &contracts::auth_runtime::AuthContext,
        workspace_id: &str,
        limit: usize,
    ) -> Result<Vec<ShareAccessLog>> {
        if !self
            .check_access(ctx, workspace_id)
            .await?
            .allows_share_management()
        {
            bail!("insufficient permission to view access logs");
        }
        Ok(self
            .store
            .get_share_access_logs(ctx, uuid::Uuid::parse_str(workspace_id)?, limit)
            .await?
            .into_iter()
            .map(|entry| ShareAccessLog {
                id: entry.id,
                workspace_id: entry.workspace_id,
                share_token: entry.share_token,
                action: entry.action,
                accessed_at: entry.accessed_at,
            })
            .collect())
    }

    pub async fn load_shared_workspace(&self, token: &str) -> Result<Option<SharedWorkspacePayload>> {
        Ok(self
            .store
            .load_shared_workspace(token)
            .await?
            .map(|snapshot| SharedWorkspacePayload {
                knowledge_base: SharedKnowledgeBase {
                    id: snapshot.knowledge_base.id,
                    title: snapshot.knowledge_base.title,
                    description: snapshot.knowledge_base.description,
                },
                share: SharedShareInfo {
                    permission: snapshot.share.permission,
                    expires_at: snapshot.share.expires_at,
                    allow_download: snapshot.share.allow_download,
                    scope: snapshot.share.scope,
                },
                sources: snapshot
                    .sources
                    .into_iter()
                    .map(|source| SharedSource {
                        id: source.id,
                        file_name: source.file_name,
                        status: source.status,
                    })
                    .collect(),
            }))
    }

    pub async fn resolve_public_share_chat_context(
        &self,
        token: &str,
    ) -> Result<Option<PublicShareChatContext>> {
        Ok(self
            .store
            .resolve_public_share_chat_context(token)
            .await?
            .map(|snapshot| PublicShareChatContext {
                owner_user_id: snapshot.owner_user_id,
                workspace_id: snapshot.workspace_id,
                access_level: snapshot.access_level.into(),
            }))
    }
}
