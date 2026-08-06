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

    /// Call MCP tool; returns `structuredContent` on success.
    pub async fn tools_call(&self, tool: &str, arguments: Value) -> Result<Value> {
        let key = self.cfg.require_api_key()?;
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
            .bearer_auth(key)
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
}
