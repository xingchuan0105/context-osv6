//! Env / defaults for local agent client (MCP + CLI).

use anyhow::{Result, bail};

pub const DEFAULT_API_BASE: &str = "http://127.0.0.1:18080";
pub const MCP_RPC_PATH: &str = "/api/v1/mcp";
pub const HEALTH_PATH: &str = "/health";

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub api_base: String,
    /// Workspace API key (index/query automation).
    pub api_key: Option<String>,
    /// User JWT / short-lived agent token (account tools, create workspace).
    pub user_token: Option<String>,
    pub workspace_id: Option<String>,
    pub mcp_url: String,
    pub health_url: String,
}

impl ClientConfig {
    pub fn from_env() -> Result<Self> {
        let api_base = first_nonempty_env(&["CONTEXT_OS_API_BASE", "AVRAG_PUBLIC_BASE_URL"])
            .unwrap_or_else(|| DEFAULT_API_BASE.to_string());
        let api_base = normalize_base(&api_base);
        if api_base.is_empty() {
            bail!("API base URL is empty");
        }

        let api_key =
            first_nonempty_env(&["CONTEXT_OS_API_KEY", "CONTEXT_OS_WORKSPACE_API_KEY"]);
        let user_token = first_nonempty_env(&[
            "CONTEXT_OS_USER_TOKEN",
            "CONTEXT_OS_AGENT_TOKEN",
            "CONTEXT_OS_JWT",
        ]);
        let workspace_id = first_nonempty_env(&["CONTEXT_OS_WORKSPACE_ID", "CONTEXT_OS_NOTEBOOK_ID"]);

        Ok(Self {
            mcp_url: format!("{api_base}{MCP_RPC_PATH}"),
            health_url: format!("{api_base}{HEALTH_PATH}"),
            api_base,
            api_key,
            user_token,
            workspace_id,
        })
    }

    pub fn with_api_base(mut self, base: Option<String>) -> Self {
        if let Some(base) = base.filter(|b| !b.trim().is_empty()) {
            let api_base = normalize_base(&base);
            self.mcp_url = format!("{api_base}{MCP_RPC_PATH}");
            self.health_url = format!("{api_base}{HEALTH_PATH}");
            self.api_base = api_base;
        }
        self
    }

    pub fn with_api_key(mut self, key: Option<String>) -> Self {
        if let Some(key) = key.filter(|k| !k.trim().is_empty()) {
            self.api_key = Some(key);
        }
        self
    }

    pub fn with_user_token(mut self, token: Option<String>) -> Self {
        if let Some(token) = token.filter(|t| !t.trim().is_empty()) {
            self.user_token = Some(token);
        }
        self
    }

    pub fn with_workspace_id(mut self, id: Option<String>) -> Self {
        if let Some(id) = id.filter(|s| !s.trim().is_empty()) {
            self.workspace_id = Some(id);
        }
        self
    }

    pub fn has_api_key(&self) -> bool {
        self.api_key
            .as_ref()
            .is_some_and(|k| !k.trim().is_empty())
    }

    pub fn has_user_token(&self) -> bool {
        self.user_token
            .as_ref()
            .is_some_and(|t| !t.trim().is_empty())
    }

    /// Prefer user JWT for MCP/API calls when set (full personal capabilities).
    pub fn bearer_token(&self) -> Option<&str> {
        self.user_token
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .or_else(|| {
                self.api_key
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
            })
    }

    pub fn require_bearer(&self) -> Result<&str> {
        self.bearer_token().ok_or_else(|| {
            anyhow::anyhow!(
                "{} Also set CONTEXT_OS_USER_TOKEN for create_workspace / user-session routes \
(mint via POST /api/auth/agent-token while signed in).",
                missing_key_message()
            )
        })
    }

    pub fn require_user_token(&self) -> Result<&str> {
        match self
            .user_token
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(t) => Ok(t),
            None => bail!(
                "CONTEXT_OS_USER_TOKEN required (user JWT or short-lived agent token). \
Login with `context-os auth login`, or mint with `context-os auth mint` while holding a session JWT. \
Workspace API keys cannot create workspaces or manage share."
            ),
        }
    }

    pub fn require_api_key(&self) -> Result<&str> {
        // Prefer any bearer for workspace tools (user JWT works too).
        self.require_bearer()
    }

    pub fn require_workspace_id(&self) -> Result<&str> {
        match self
            .workspace_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(id) => Ok(id),
            None => bail!(
                "workspace_id required: pass --workspace <uuid> or set CONTEXT_OS_WORKSPACE_ID"
            ),
        }
    }
}

/// Backward-compatible alias used by the stdio MCP binary.
pub type McpProxyConfig = ClientConfig;

pub fn first_nonempty_env(keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Ok(v) = std::env::var(key) {
            let t = v.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

fn normalize_base(raw: &str) -> String {
    raw.trim().trim_end_matches('/').to_string()
}

/// Human-readable guidance when the local API is unreachable.
pub fn unreachable_message(api_base: &str, detail: &str) -> String {
    format!(
        "Context-OS local API is not reachable at {api_base} ({detail}). \
Start the Context-OS desktop client (or avrag-api on this host) and confirm \
AVRAG_API_ADDR / CONTEXT_OS_API_BASE. Default local base is {DEFAULT_API_BASE}."
    )
}

pub fn missing_key_message() -> String {
    "CONTEXT_OS_API_KEY (or CONTEXT_OS_WORKSPACE_API_KEY) is not set. \
Create a workspace API key in the client UI (API Access), then export it. \
Workspace keys need `index` and/or `query` permissions."
        .to_string()
}

pub fn unauthorized_message(status: u16) -> String {
    format!(
        "Local API rejected credentials (HTTP {status}). \
Check CONTEXT_OS_API_KEY is a workspace API key for the workspace_id you pass, \
not a user password. Create keys under Workspace → API Access."
    )
}

pub fn share_forbidden_message() -> String {
    "Share requires CONTEXT_OS_USER_TOKEN (user JWT / agent token), not a workspace API key. \
Use: context-os auth mint && context-os share enable --workspace <id>. \
Quotas follow the owner subscription (ADR-0010)."
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_trailing_slash() {
        assert_eq!(
            normalize_base("http://127.0.0.1:18080/"),
            "http://127.0.0.1:18080"
        );
        assert_eq!(normalize_base("  http://x  "), "http://x");
    }

    #[test]
    fn unreachable_mentions_default_and_start_client() {
        let msg = unreachable_message("http://127.0.0.1:18080", "connection refused");
        assert!(msg.contains("connection refused"));
        assert!(msg.contains("desktop client") || msg.contains("avrag-api"));
        assert!(msg.contains(DEFAULT_API_BASE));
    }

    #[test]
    fn missing_key_mentions_api_access() {
        let msg = missing_key_message();
        assert!(msg.contains("CONTEXT_OS_API_KEY"));
        assert!(msg.contains("API Access"));
    }

    #[test]
    fn share_message_blocks_api_key_path() {
        let msg = share_forbidden_message();
        assert!(msg.contains("API key") || msg.contains("workspace"));
        assert!(msg.contains("Share"));
    }
}
