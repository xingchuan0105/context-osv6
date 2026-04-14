//! Share API client
#![allow(dead_code)]

use crate::{ApiClient, dtos::*};
use anyhow::{anyhow, bail};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
struct ApiErrorEnvelope {
    message: String,
}

#[derive(Debug, Deserialize)]
struct ApiEnvelope<T> {
    #[serde(default)]
    ok: bool,
    data: Option<T>,
    error: Option<ApiErrorEnvelope>,
}

#[derive(Debug, Deserialize)]
struct RawShareTokenInfo {
    token: String,
    access_level: String,
    expires_at: Option<String>,
    revoked_at: Option<String>,
    access_count: i64,
}

#[derive(Debug, Deserialize)]
struct RawNotebookMember {
    id: String,
    notebook_id: String,
    user_id: Option<String>,
    email: Option<String>,
    access_level: serde_json::Value,
    invite_status: String,
    invited_by: Option<String>,
    invited_at: i64,
    accepted_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RawShareSettings {
    access_level: String,
    allow_download: bool,
    share_tokens: Vec<RawShareTokenInfo>,
    members: Vec<RawNotebookMember>,
}

#[derive(Debug, Deserialize)]
struct RawShareAnalytics {
    token: String,
    access_level: String,
    total_views: i64,
    last_accessed_at: Option<i64>,
    created_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawShareAccessLog {
    id: String,
    notebook_id: String,
    share_token: String,
    action: String,
    accessed_at: i64,
}

#[derive(Debug, Deserialize)]
struct SharedNotebookEnvelope {
    success: bool,
    data: Option<SharedNotebookPayload>,
    error: Option<String>,
}

fn unwrap_api_data<T>(envelope: ApiEnvelope<T>, fallback: &str) -> anyhow::Result<T> {
    if envelope.ok {
        return envelope
            .data
            .ok_or_else(|| anyhow!("missing data in API response"));
    }

    let message = envelope
        .error
        .map(|err| err.message)
        .unwrap_or_else(|| fallback.to_string());
    bail!(message)
}

impl ApiClient {
    /// POST /api/v1/notebooks/{notebook_id}/share
    pub async fn create_share(&self, notebook_id: &str) -> anyhow::Result<ShareTokenResponse> {
        self.create_share_with_options(notebook_id, "viewer", None)
            .await
    }

    /// DELETE /api/v1/notebooks/{notebook_id}/share/{token}
    pub async fn revoke_share(
        &self,
        notebook_id: &str,
        token: &str,
    ) -> anyhow::Result<EmptyResponse> {
        self.delete(&format!(
            "/api/v1/notebooks/{}/share/{}",
            notebook_id, token
        ))
        .await
    }

    pub async fn create_share_with_options(
        &self,
        notebook_id: &str,
        role: &str,
        expires_at: Option<String>,
    ) -> anyhow::Result<ShareTokenResponse> {
        #[derive(serde::Serialize)]
        struct Body {
            role: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            expires_at: Option<String>,
        }
        self.post(
            &format!("/api/v1/notebooks/{}/share", notebook_id),
            &Body {
                role: role.to_string(),
                expires_at,
            },
        )
        .await
    }

    /// GET /api/v1/notebooks/{notebook_id}/share/settings
    pub async fn get_share_settings(&self, notebook_id: &str) -> anyhow::Result<ShareSettings> {
        let raw: RawShareSettings = self
            .get(&format!("/api/v1/notebooks/{}/share/settings", notebook_id))
            .await?;
        let share_token = raw
            .share_tokens
            .iter()
            .find(|token| token.revoked_at.is_none())
            .or_else(|| raw.share_tokens.first());
        Ok(ShareSettings {
            share_token: share_token
                .map(|token| token.token.clone())
                .unwrap_or_default(),
            access_level: raw.access_level,
            expires_at: share_token.and_then(|token| token.expires_at.clone()),
            allow_download: raw.allow_download,
        })
    }

    /// POST /api/v1/notebooks/{notebook_id}/access-level
    pub async fn set_access_level(
        &self,
        notebook_id: &str,
        access_level: &str,
    ) -> anyhow::Result<EmptyResponse> {
        #[derive(serde::Serialize)]
        struct Body {
            access_level: String,
        }
        let _: serde_json::Value = self
            .post(
                &format!("/api/v1/notebooks/{}/access-level", notebook_id),
                &Body {
                    access_level: access_level.to_string(),
                },
            )
            .await?;
        Ok(EmptyResponse {})
    }

    /// PUT /api/v1/notebooks/{notebook_id}/share/settings
    pub async fn update_share_settings(
        &self,
        notebook_id: &str,
        settings: &ShareSettings,
    ) -> anyhow::Result<ShareSettings> {
        #[derive(serde::Serialize)]
        struct Body<'a> {
            access_level: &'a str,
            allow_download: bool,
        }
        let raw: RawShareSettings = self
            .put(
                &format!("/api/v1/notebooks/{}/share/settings", notebook_id),
                &Body {
                    access_level: &settings.access_level,
                    allow_download: settings.allow_download,
                },
            )
            .await?;
        let share_token = raw
            .share_tokens
            .iter()
            .find(|token| token.revoked_at.is_none())
            .or_else(|| raw.share_tokens.first());
        Ok(ShareSettings {
            share_token: share_token
                .map(|token| token.token.clone())
                .unwrap_or_default(),
            access_level: raw.access_level,
            expires_at: share_token.and_then(|token| token.expires_at.clone()),
            allow_download: raw.allow_download,
        })
    }

    /// GET /api/v1/notebooks/{notebook_id}/share/analytics
    pub async fn get_share_analytics(
        &self,
        notebook_id: &str,
    ) -> anyhow::Result<ShareAnalyticsResponse> {
        let envelope: ApiEnvelope<Vec<RawShareAnalytics>> = self
            .get(&format!(
                "/api/v1/notebooks/{}/share/analytics",
                notebook_id
            ))
            .await?;
        let entries = unwrap_api_data(envelope, "failed to load share analytics")?;
        let total_views = entries.iter().map(|entry| entry.total_views).sum();
        let total_unique_visitors = entries.len() as i64;
        let mut views_by_day = BTreeMap::<String, i64>::new();
        for entry in entries {
            let day = entry
                .created_at
                .as_deref()
                .unwrap_or("unknown")
                .chars()
                .take(10)
                .collect::<String>();
            *views_by_day.entry(day).or_insert(0) += entry.total_views;
        }
        Ok(ShareAnalyticsResponse {
            total_views,
            total_unique_visitors,
            views_by_day,
        })
    }

    /// GET /api/v1/notebooks/{notebook_id}/share/access-logs
    pub async fn get_access_logs(&self, notebook_id: &str) -> anyhow::Result<AccessLogsResponse> {
        let envelope: ApiEnvelope<Vec<RawShareAccessLog>> = self
            .get(&format!(
                "/api/v1/notebooks/{}/share/access-logs",
                notebook_id
            ))
            .await?;
        let logs = unwrap_api_data(envelope, "failed to load share access logs")?
            .into_iter()
            .map(|raw| AccessLogEntry {
                id: raw.id,
                visitor_id: raw.share_token,
                accessed_at: raw.accessed_at.to_string(),
                action: raw.action,
            })
            .collect();
        Ok(AccessLogsResponse { logs })
    }

    /// GET /api/v1/share/validate/{token}
    pub async fn validate_share_token(&self, token: &str) -> anyhow::Result<ShareTokenResponse> {
        let envelope: ApiEnvelope<ShareTokenResponse> = self
            .get(&format!("/api/v1/share/validate/{}", token))
            .await?;
        unwrap_api_data(envelope, "failed to validate share token")
    }

    /// GET /api/shared/kb/{token}
    pub async fn get_shared_kb(&self, token: &str) -> anyhow::Result<SharedNotebookPayload> {
        let envelope: SharedNotebookEnvelope =
            self.get(&format!("/api/shared/kb/{}", token)).await?;
        if envelope.success {
            return envelope
                .data
                .ok_or_else(|| anyhow!("missing shared notebook payload"));
        }
        bail!(
            envelope
                .error
                .unwrap_or_else(|| "failed to load shared notebook".to_string())
        )
    }
}
