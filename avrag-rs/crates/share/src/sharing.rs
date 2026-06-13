use anyhow::{bail, Result};
use avrag_auth::AuthContext;
use chrono::{Duration, Utc};
use tracing::warn;
use uuid::Uuid;

use crate::{AccessLevel, ShareAnalytics, ShareService, ShareSettings, ShareTokenInfo};

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
        if let Err(error) = self.store.record_share_product_event(event).await {
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
        let (access_level, allow_download, share_tokens) = self
            .store
            .get_share_settings(ctx, notebook_uuid)
            .await?;
        Ok(ShareSettings {
            access_level,
            allow_download,
            share_tokens: share_tokens
                .into_iter()
                .map(|token| ShareTokenInfo {
                    token: token.token,
                    access_level: token.access_level,
                    expires_at: token.expires_at,
                    revoked_at: token.revoked_at,
                    access_count: token.access_count,
                })
                .collect(),
            members: self
                .list_members(ctx, notebook_id)
                .await?
                .into_iter()
                .collect(),
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
        self.store
            .update_notebook_access_level(ctx, Uuid::parse_str(notebook_id)?, normalized)
            .await?;
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
        self.store
            .update_share_settings(
                ctx,
                Uuid::parse_str(notebook_id)?,
                normalized_access_level.as_deref(),
                allow_download,
            )
            .await?;
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
        let expires_at = expires_in_secs.map(|secs| Utc::now() + Duration::seconds(secs));
        let token = self
            .store
            .create_share_token(
                ctx,
                Uuid::parse_str(notebook_id)?,
                access_level.into(),
                expires_at,
            )
            .await?;
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
        Ok(self
            .store
            .validate_token(token)
            .await?
            .map(|(notebook_id, level)| (notebook_id.to_string(), level.into())))
    }

    pub async fn revoke_token(&self, ctx: &AuthContext, token: &str) -> Result<()> {
        let Some((notebook_id, _)) = self.validate_token(token).await? else {
            return Ok(());
        };
        if !self
            .check_access(ctx, &notebook_id)
            .await?
            .allows_share_management()
        {
            bail!("insufficient permission to revoke share link");
        }
        let _ = self.store.revoke_token(ctx, token).await?;
        self.record_share_event(
            ctx,
            &notebook_id,
            analytics::ProductEventName::ShareLinkDisabled,
            serde_json::json!({ "token": token }),
        )
        .await;
        Ok(())
    }

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
        Ok(self
            .store
            .get_share_analytics(ctx, Uuid::parse_str(notebook_id)?)
            .await?
            .into_iter()
            .map(|entry| ShareAnalytics {
                token: entry.token,
                access_level: entry.access_level,
                total_views: entry.total_views,
                last_accessed_at: entry.last_accessed_at,
                created_at: entry.created_at,
            })
            .collect())
    }
}
