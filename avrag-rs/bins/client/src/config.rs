//! Env / defaults for local agent client (MCP + CLI).

use anyhow::{Result, bail};

pub const DEFAULT_API_BASE: &str = "http://127.0.0.1:18080";
pub const MCP_RPC_PATH: &str = "/api/v1/mcp";
pub const HEALTH_PATH: &str = "/health";

/// Where a user JWT came from (drives least-privilege bearer selection).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UserTokenSource {
    #[default]
    None,
    /// Explicit env or CLI `--user-token`.
    Explicit,
    /// Loaded from `user.token` file (auto).
    TokenFile,
    /// Loaded from desktop `local_session.json` (auto).
    Desktop,
}

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub api_base: String,
    /// Workspace API key (index/query automation).
    pub api_key: Option<String>,
    /// User JWT / short-lived agent token (account tools, create workspace).
    pub user_token: Option<String>,
    pub user_token_source: UserTokenSource,
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

        let mut user_token = first_nonempty_env(&[
            "CONTEXT_OS_USER_TOKEN",
            "CONTEXT_OS_AGENT_TOKEN",
            "CONTEXT_OS_JWT",
        ]);
        let mut user_token_source = UserTokenSource::None;
        if user_token.is_some() {
            user_token_source = UserTokenSource::Explicit;
        } else {
            // Token file first (intentional --save). Desktop only if explicitly enabled.
            if let Some(t) = crate::token_store::load_default_token_file()? {
                user_token = Some(t);
                user_token_source = UserTokenSource::TokenFile;
            } else {
                // Default OFF: avoid elevating API-key-only agents via desktop login JWT.
                let load_desktop = match std::env::var("CONTEXT_OS_LOAD_DESKTOP_SESSION")
                    .unwrap_or_else(|_| "0".into())
                    .to_ascii_lowercase()
                    .as_str()
                {
                    "1" | "true" | "yes" | "on" => true,
                    _ => false,
                };
                if load_desktop {
                    if let Some(s) = crate::token_store::load_desktop_session_token()? {
                        user_token = Some(s.token);
                        user_token_source = UserTokenSource::Desktop;
                    }
                }
            }
        }

        let workspace_id = first_nonempty_env(&["CONTEXT_OS_WORKSPACE_ID", "CONTEXT_OS_NOTEBOOK_ID"]);

        Ok(Self {
            mcp_url: format!("{api_base}{MCP_RPC_PATH}"),
            health_url: format!("{api_base}{HEALTH_PATH}"),
            api_base,
            api_key,
            user_token,
            user_token_source,
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
            self.user_token_source = UserTokenSource::Explicit;
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

    /// True when user JWT was auto-discovered (token file or desktop), not env/CLI.
    pub fn user_token_is_auto_discovered(&self) -> bool {
        matches!(
            self.user_token_source,
            UserTokenSource::TokenFile | UserTokenSource::Desktop
        )
    }

    /// Select Bearer for general MCP/REST.
    ///
    /// Least privilege: when a workspace API key is set and the user JWT was only
    /// auto-discovered, prefer the API key so share/account elevation is not silent.
    /// Explicit `CONTEXT_OS_USER_TOKEN` / `--user-token` still wins over API key.
    pub fn bearer_token(&self) -> Option<&str> {
        let user = self
            .user_token
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let key = self
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());

        match (key, user, self.user_token_source) {
            (Some(k), Some(_), UserTokenSource::TokenFile | UserTokenSource::Desktop) => Some(k),
            (_, Some(u), _) => Some(u),
            (Some(k), None, _) => Some(k),
            _ => None,
        }
    }

    pub fn credential_source_label(&self) -> &'static str {
        let using = self.bearer_token();
        let user = self
            .user_token
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let key = self
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        match (using, user, key, self.user_token_source) {
            (Some(u), Some(ut), _, UserTokenSource::Explicit) if u == ut => "user_token(explicit)",
            (Some(u), Some(ut), _, UserTokenSource::TokenFile) if u == ut => "user_token(file)",
            (Some(u), Some(ut), _, UserTokenSource::Desktop) if u == ut => "user_token(desktop)",
            (Some(k), Some(_), Some(ak), UserTokenSource::TokenFile | UserTokenSource::Desktop)
                if k == ak =>
            {
                "workspace_api_key(over auto user)"
            }
            (Some(k), _, Some(ak), _) if k == ak => "workspace_api_key",
            (Some(_), _, _, _) => "credentials",
            _ => "none",
        }
    }

    pub fn require_bearer(&self) -> Result<&str> {
        self.bearer_token().ok_or_else(|| {
            anyhow::anyhow!(
                "{} Or set CONTEXT_OS_USER_TOKEN / run `context-os auth from-desktop --save` \
or `auth mint --save`.",
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
            None => bail!("{}", user_session_required_message()),
        }
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
    "No credentials: set CONTEXT_OS_API_KEY (workspace index/query) and/or \
CONTEXT_OS_USER_TOKEN (user session). Create workspace keys under API Access; \
user tokens via `context-os auth mint --save` or `auth from-desktop --save`."
        .to_string()
}

pub fn unauthorized_message(status: u16) -> String {
    format!(
        "Local API rejected credentials (HTTP {status}). \
Check CONTEXT_OS_API_KEY is a workspace API key for the workspace_id you pass, \
or CONTEXT_OS_USER_TOKEN is a valid user JWT. Create keys under Workspace → API Access."
    )
}

pub fn share_forbidden_message() -> String {
    user_session_required_message()
}

pub fn user_session_required_message() -> String {
    format!(
        "User session required (CONTEXT_OS_USER_TOKEN / agent token), not a workspace API key. \
Try: `context-os auth from-desktop --save` (set CONTEXT_OS_LOAD_DESKTOP_SESSION=1 to auto-load), \
`auth login --save`, or `auth mint --save`. Token file: {}.",
        crate::token_store::default_token_file().display()
    )
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
    fn missing_key_mentions_credentials() {
        let msg = missing_key_message();
        assert!(msg.contains("CONTEXT_OS_API_KEY") || msg.contains("CONTEXT_OS_USER_TOKEN"));
    }

    #[test]
    fn share_message_blocks_api_key_path() {
        let msg = share_forbidden_message();
        assert!(msg.contains("API key") || msg.contains("workspace"));
        assert!(msg.contains("User session") || msg.contains("USER_TOKEN"));
    }

    #[test]
    fn bearer_prefers_api_key_over_auto_user_token() {
        let cfg = ClientConfig {
            api_base: "http://127.0.0.1:18080".into(),
            api_key: Some("wk_xxx".into()),
            user_token: Some("jwt.auto".into()),
            user_token_source: UserTokenSource::Desktop,
            workspace_id: None,
            mcp_url: "http://127.0.0.1:18080/api/v1/mcp".into(),
            health_url: "http://127.0.0.1:18080/health".into(),
        };
        assert_eq!(cfg.bearer_token(), Some("wk_xxx"));
        assert_eq!(cfg.require_user_token().unwrap(), "jwt.auto");
    }

    #[test]
    fn bearer_prefers_explicit_user_token_over_api_key() {
        let cfg = ClientConfig {
            api_base: "http://127.0.0.1:18080".into(),
            api_key: Some("wk_xxx".into()),
            user_token: Some("jwt.explicit".into()),
            user_token_source: UserTokenSource::Explicit,
            workspace_id: None,
            mcp_url: "http://127.0.0.1:18080/api/v1/mcp".into(),
            health_url: "http://127.0.0.1:18080/health".into(),
        };
        assert_eq!(cfg.bearer_token(), Some("jwt.explicit"));
    }

    #[test]
    fn bearer_uses_auto_user_when_no_api_key() {
        let cfg = ClientConfig {
            api_base: "http://127.0.0.1:18080".into(),
            api_key: None,
            user_token: Some("jwt.file".into()),
            user_token_source: UserTokenSource::TokenFile,
            workspace_id: None,
            mcp_url: "http://127.0.0.1:18080/api/v1/mcp".into(),
            health_url: "http://127.0.0.1:18080/health".into(),
        };
        assert_eq!(cfg.bearer_token(), Some("jwt.file"));
    }
}
