use anyhow::{bail, Result};

use crate::{
    PublicOwnerShareItem, PublicShareChatContext, ShareAccessLog, ShareOwnerCard, ShareService,
    SharedKnowledgeBase, SharedWorkspacePayload, SharedShareInfo, SharedSource,
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
                owner: snapshot.owner.map(|card| ShareOwnerCard {
                    user_id: card.user_id,
                    display_name: card.display_name,
                    bio: card.bio,
                    contact_url: card.contact_url,
                    avatar_url: card.avatar_url,
                    banner_url: card.banner_url,
                    profile_enabled: card.profile_enabled,
                }),
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
                workspace_visibility: snapshot.workspace_visibility,
                share_enabled: snapshot.share_enabled,
                anon_question_limit: snapshot.anon_question_limit,
                member_question_limit: snapshot.member_question_limit,
            }))
    }

    pub async fn list_public_shares_for_owner(
        &self,
        user_id: uuid::Uuid,
    ) -> Result<Vec<PublicOwnerShareItem>> {
        Ok(self
            .store
            .list_public_shares_for_owner(user_id)
            .await?
            .into_iter()
            .map(|item| PublicOwnerShareItem {
                workspace_id: item.workspace_id,
                title: item.title,
                description: item.description,
                share_token: item.share_token,
                access_level: item.access_level,
                allow_download: item.allow_download,
                source_count: item.source_count,
            })
            .collect())
    }
}
