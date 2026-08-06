//! Local API discovery and readable status / `--check`.

use crate::config::{self, ClientConfig};

/// Probe `/health` and optionally MCP initialize. Exit code: 0 ok, 1 unreachable/auth fail, 3 missing key.
pub async fn run_check(cfg: &ClientConfig) -> Result<(), u8> {
    eprintln!("context-os check");
    eprintln!("  api_base:  {}", cfg.api_base);
    eprintln!("  mcp_url:   {}", cfg.mcp_url);
    eprintln!(
        "  api_key:   {}",
        if cfg.has_api_key() {
            "set"
        } else {
            "missing"
        }
    );
    if let Some(ws) = cfg.workspace_id.as_deref() {
        eprintln!("  workspace: {ws}");
    }

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("  health:    error building HTTP client: {e}");
            return Err(1);
        }
    };

    match client.get(&cfg.health_url).send().await {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if status.is_success() {
                eprintln!("  health:    OK ({status}) {body}");
            } else {
                eprintln!("  health:    unexpected HTTP {status}: {body}");
                eprintln!("  hint:      API process is up but /health failed — check logs.");
                return Err(1);
            }
        }
        Err(e) => {
            let detail = if e.is_connect() {
                "connection refused or host unreachable".to_string()
            } else if e.is_timeout() {
                "timeout".to_string()
            } else {
                e.to_string()
            };
            eprintln!("  health:    FAIL — {detail}");
            eprintln!(
                "  hint:      {}",
                config::unreachable_message(&cfg.api_base, &detail)
            );
            return Err(1);
        }
    }

    if !cfg.has_api_key() {
        eprintln!("  auth:      WARN — {}", config::missing_key_message());
        return Err(3);
    }

    let init_body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "check",
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "context-os-check", "version": "0.1.0" }
        }
    });

    let mut req = client
        .post(&cfg.mcp_url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json");
    if let Some(key) = cfg.api_key.as_ref() {
        req = req.bearer_auth(key);
    }

    match req.json(&init_body).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            if status == 401 || status == 403 {
                eprintln!("  mcp auth:  FAIL HTTP {status}");
                eprintln!("  hint:      {}", config::unauthorized_message(status));
                return Err(1);
            }
            if !(200..300).contains(&status) {
                eprintln!("  mcp auth:  FAIL HTTP {status}: {text}");
                return Err(1);
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if v.get("error").is_some() && v.get("result").is_none() {
                    eprintln!("  mcp auth:  JSON-RPC error: {text}");
                    return Err(1);
                }
                if v.pointer("/result/serverInfo/name").and_then(|n| n.as_str())
                    == Some("context-os")
                {
                    eprintln!("  mcp auth:  OK (initialize → context-os)");
                } else {
                    eprintln!("  mcp auth:  OK HTTP {status}");
                }
            } else {
                eprintln!("  mcp auth:  OK HTTP {status} (non-JSON body)");
            }
        }
        Err(e) => {
            eprintln!("  mcp auth:  FAIL — {e}");
            return Err(1);
        }
    }

    eprintln!("  result:    ready (upload + query via workspace tools / CLI)");
    Ok(())
}
