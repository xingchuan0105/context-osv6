//! Thin HTTP client for unified MCP `tools/call`.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::config::{self, ClientConfig};

#[derive(Debug)]
pub struct McpClient {
    http: reqwest::Client,
    cfg: ClientConfig,
}

impl McpClient {
    pub fn new(cfg: ClientConfig) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(180))
            .build()
            .context("build HTTP client")?;
        Ok(Self { http, cfg })
    }

    pub fn config(&self) -> &ClientConfig {
        &self.cfg
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// Call MCP tool with least-privilege bearer (API key preferred when user JWT is auto-loaded).
    /// Use for index/query automation.
    pub async fn tools_call(&self, tool: &str, arguments: Value) -> Result<Value> {
        let bearer = self.cfg.require_bearer()?;
        self.tools_call_with_bearer(tool, arguments, bearer).await
    }

    /// Call MCP tool with an explicit **user** JWT (account / share tools).
    ///
    /// Required when both a workspace API key and an auto-loaded `user.token` are present:
    /// least-privilege `bearer_token()` would send the API key and hit `api_key_forbidden`.
    pub async fn tools_call_as_user(&self, tool: &str, arguments: Value) -> Result<Value> {
        let bearer = self.cfg.require_user_token()?;
        self.tools_call_with_bearer(tool, arguments, bearer).await
    }

    async fn tools_call_with_bearer(
        &self,
        tool: &str,
        arguments: Value,
        bearer: &str,
    ) -> Result<Value> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": "1",
            "method": "tools/call",
            "params": {
                "name": tool,
                "arguments": arguments,
            }
        });

        let resp = self
            .http
            .post(&self.cfg.mcp_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .bearer_auth(bearer)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                let detail = if e.is_connect() {
                    "connection refused or host unreachable".to_string()
                } else if e.is_timeout() {
                    "timeout".to_string()
                } else {
                    e.to_string()
                };
                anyhow::anyhow!(config::unreachable_message(&self.cfg.api_base, &detail))
            })?;

        let status = resp.status().as_u16();
        let text = resp.text().await.context("read MCP response body")?;

        if status == 401 || status == 403 {
            bail!("{}", config::unauthorized_message(status));
        }
        if !(200..300).contains(&status) {
            bail!("MCP gateway HTTP {status}: {text}");
        }

        let envelope: Value = serde_json::from_str(&text)
            .with_context(|| format!("invalid MCP JSON: {text}"))?;

        if let Some(err) = envelope.get("error") {
            let code = err
                .pointer("/data/error")
                .and_then(|v| v.as_str())
                .or_else(|| err.get("message").and_then(|v| v.as_str()))
                .unwrap_or("mcp_error");
            let message = err
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("tools/call failed");
            if matches!(
                code,
                "api_key_forbidden"
                    | "workspace_key_cannot_call_account_tools"
                    | "workspace_key_cannot_call_org_tools"
            ) {
                bail!(
                    "MCP error `{code}`: {} ({message})",
                    config::user_session_required_message()
                );
            }
            bail!("MCP error `{code}`: {message}");
        }

        if let Some(structured) = envelope.pointer("/result/structuredContent").cloned() {
            return Ok(structured);
        }
        if let Some(result) = envelope.get("result").cloned() {
            return Ok(result);
        }
        bail!("MCP tools/call missing result: {text}");
    }

    /// Resolve upload URL that may be absolute or path-relative to api_base.
    pub fn resolve_url(&self, upload_url: &str) -> String {
        let u = upload_url.trim();
        if u.starts_with("http://") || u.starts_with("https://") {
            return u.to_string();
        }
        if u.starts_with('/') {
            return format!("{}{u}", self.cfg.api_base);
        }
        format!("{}/{}", self.cfg.api_base, u)
    }

    pub async fn put_bytes(&self, url: &str, bytes: Vec<u8>) -> Result<()> {
        let resp = self
            .http
            .put(url)
            .header("Content-Type", "application/octet-stream")
            .body(bytes)
            .send()
            .await
            .with_context(|| format!("PUT upload to {url}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("upload PUT failed HTTP {status}: {body}");
        }
        Ok(())
    }

    /// REST JSON with preferred bearer (user token > api key).
    pub async fn rest_json(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        require_user: bool,
    ) -> Result<(u16, Value)> {
        let bearer = if require_user {
            self.cfg.require_user_token()?
        } else {
            self.cfg.require_bearer()?
        };
        let url = if path.starts_with("http://") || path.starts_with("https://") {
            path.to_string()
        } else if path.starts_with('/') {
            format!("{}{path}", self.cfg.api_base)
        } else {
            format!("{}/{}", self.cfg.api_base, path)
        };

        let mut req = match method.to_uppercase().as_str() {
            "GET" => self.http.get(&url),
            "POST" => self.http.post(&url),
            "PUT" => self.http.put(&url),
            "DELETE" => self.http.delete(&url),
            other => bail!("unsupported method {other}"),
        };
        req = req
            .header("Accept", "application/json")
            .bearer_auth(bearer);
        if let Some(b) = body {
            req = req.header("Content-Type", "application/json").json(&b);
        }

        let resp = req.send().await.with_context(|| format!("{method} {url}"))?;
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        let value = serde_json::from_str(&text).unwrap_or_else(|_| json!({ "raw": text }));
        Ok((status, value))
    }
}

/// Prefer structured `data` field when present.
pub fn tool_data(structured: &Value) -> &Value {
    structured.get("data").unwrap_or(structured)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_relative_and_absolute() {
        let cfg = ClientConfig {
            api_base: "http://127.0.0.1:18080".into(),
            api_key: None,
            user_token: None,
            user_token_source: crate::config::UserTokenSource::None,
            workspace_id: None,
            mcp_url: "http://127.0.0.1:18080/api/v1/mcp".into(),
            health_url: "http://127.0.0.1:18080/health".into(),
        };
        let c = McpClient {
            http: reqwest::Client::new(),
            cfg,
        };
        assert_eq!(
            c.resolve_url("/dev-upload/abc"),
            "http://127.0.0.1:18080/dev-upload/abc"
        );
        assert_eq!(
            c.resolve_url("https://cdn.example/u"),
            "https://cdn.example/u"
        );
    }

    #[test]
    fn dual_cred_bearer_vs_user_selection() {
        use crate::config::UserTokenSource;
        let cfg = ClientConfig {
            api_base: "http://127.0.0.1:18080".into(),
            api_key: Some("wk_key".into()),
            user_token: Some("jwt.file".into()),
            user_token_source: UserTokenSource::TokenFile,
            workspace_id: None,
            mcp_url: "http://127.0.0.1:18080/api/v1/mcp".into(),
            health_url: "http://127.0.0.1:18080/health".into(),
        };
        // Least privilege for tools_call path.
        assert_eq!(cfg.bearer_token(), Some("wk_key"));
        // User session path still has the JWT.
        assert_eq!(cfg.require_user_token().unwrap(), "jwt.file");
    }
}
