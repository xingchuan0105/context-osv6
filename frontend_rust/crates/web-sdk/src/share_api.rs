use crate::conversation_api::{encode_path_segment, trim_base_url};
use crate::transport::TransportError;
use contracts::share::{
    AccessLogsResponse, ShareAnalyticsResponse, ShareSettings, ShareTokenResponse,
    SharedWorkspacePayload,
};

pub fn share_url(base_url: &str, workspace_id: &str) -> String {
    format!(
        "{}/api/v1/workspaces/{}/share",
        trim_base_url(base_url),
        encode_path_segment(workspace_id)
    )
}

pub fn share_settings_url(base_url: &str, workspace_id: &str) -> String {
    format!(
        "{}/api/v1/workspaces/{}/share/settings",
        trim_base_url(base_url),
        encode_path_segment(workspace_id)
    )
}

pub fn share_analytics_url(base_url: &str, workspace_id: &str) -> String {
    format!(
        "{}/api/v1/workspaces/{}/share/analytics",
        trim_base_url(base_url),
        encode_path_segment(workspace_id)
    )
}

pub fn share_access_logs_url(base_url: &str, workspace_id: &str) -> String {
    format!(
        "{}/api/v1/workspaces/{}/share/access-logs",
        trim_base_url(base_url),
        encode_path_segment(workspace_id)
    )
}

pub fn shared_kb_url(base_url: &str, token: &str) -> String {
    format!(
        "{}/api/shared/kb/{}",
        trim_base_url(base_url),
        encode_path_segment(token)
    )
}

pub fn public_user_shares_url(base_url: &str, user_id: &str) -> String {
    format!(
        "{}/api/public/users/{}/shares",
        trim_base_url(base_url),
        encode_path_segment(user_id)
    )
}

pub fn parse_share_token(body: &[u8]) -> Result<ShareTokenResponse, TransportError> {
    serde_json::from_slice(body).map_err(TransportError::from)
}

pub fn parse_share_settings(body: &[u8]) -> Result<ShareSettings, TransportError> {
    serde_json::from_slice(body).map_err(TransportError::from)
}

pub fn parse_share_analytics(body: &[u8]) -> Result<ShareAnalyticsResponse, TransportError> {
    serde_json::from_slice(body).map_err(TransportError::from)
}

pub fn parse_access_logs(body: &[u8]) -> Result<AccessLogsResponse, TransportError> {
    serde_json::from_slice(body).map_err(TransportError::from)
}

pub fn parse_shared_workspace(body: &[u8]) -> Result<SharedWorkspacePayload, TransportError> {
    serde_json::from_slice(body).map_err(TransportError::from)
}
