//! stdio JSON-RPC loop → POST /api/v1/mcp

use std::io::{Write, stdout};

use anyhow::{Context, Result, bail};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::config::{self, ClientConfig};

/// True when this JSON-RPC message is a notification (no response expected).
fn is_notification(msg: &Value) -> bool {
    match msg.get("id") {
        None => true,
        Some(Value::Null) => true,
        _ => false,
    }
}

fn method_name(msg: &Value) -> &str {
    msg.get("method").and_then(|m| m.as_str()).unwrap_or("")
}

/// Local-only handling: known client notifications that need no backend hop.
fn handle_local_notification(method: &str) -> bool {
    matches!(
        method,
        "notifications/initialized" | "notifications/cancelled" | "initialized"
    )
}

fn compact_json_line(value: &Value) -> Result<String> {
    let s = serde_json::to_string(value).context("serialize JSON-RPC message")?;
    if s.contains('\n') {
        bail!("JSON-RPC message contained newline after compact encode");
    }
    Ok(s)
}

fn write_stdout_line(line: &str) -> Result<()> {
    let mut out = stdout().lock();
    out.write_all(line.as_bytes())?;
    out.write_all(b"\n")?;
    out.flush()?;
    Ok(())
}

fn jsonrpc_error_response(id: Option<Value>, code: i32, message: impl Into<String>) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "error": {
            "code": code,
            "message": message.into(),
        }
    })
}

fn extract_id(msg: &Value) -> Option<Value> {
    msg.get("id").cloned()
}

pub async fn run_stdio_proxy(cfg: ClientConfig) -> Result<()> {
    if cfg.bearer_token().is_none() {
        eprintln!("context-os-mcp: warning: {}", config::missing_key_message());
    } else {
        eprintln!(
            "context-os-mcp: credentials={}",
            cfg.credential_source_label()
        );
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .context("build HTTP client")?;

    match client.get(&cfg.health_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            eprintln!(
                "context-os-mcp: local API healthy at {} → {}",
                cfg.api_base, cfg.mcp_url
            );
        }
        Ok(resp) => {
            eprintln!(
                "context-os-mcp: warning: GET {} returned HTTP {}",
                cfg.health_url,
                resp.status()
            );
        }
        Err(e) => {
            let detail = if e.is_connect() {
                "connection refused or host unreachable".to_string()
            } else {
                e.to_string()
            };
            eprintln!(
                "context-os-mcp: warning: {}",
                config::unreachable_message(&cfg.api_base, &detail)
            );
        }
    }

    let stdin = tokio::io::stdin();
    let mut lines = BufReader::new(stdin).lines();

    while let Some(line) = lines.next_line().await.context("read stdin")? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let msg: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                let err = jsonrpc_error_response(None, -32700, format!("parse error: {e}"));
                write_stdout_line(&compact_json_line(&err)?)?;
                continue;
            }
        };

        let method = method_name(&msg).to_string();
        let notification = is_notification(&msg);

        if notification && handle_local_notification(&method) {
            continue;
        }

        match forward_rpc(&client, &cfg, &msg).await {
            Ok(Some(response_line)) => {
                if !notification {
                    write_stdout_line(&response_line)?;
                }
            }
            Ok(None) => {
                if !notification {
                    let err = jsonrpc_error_response(
                        extract_id(&msg),
                        -32603,
                        "empty or non-JSON response from local MCP gateway",
                    );
                    write_stdout_line(&compact_json_line(&err)?)?;
                }
            }
            Err(ForwardError::Unreachable(detail)) => {
                eprintln!(
                    "context-os-mcp: {}",
                    config::unreachable_message(&cfg.api_base, &detail)
                );
                if !notification {
                    let err = jsonrpc_error_response(
                        extract_id(&msg),
                        -32000,
                        config::unreachable_message(&cfg.api_base, &detail),
                    );
                    write_stdout_line(&compact_json_line(&err)?)?;
                }
            }
            Err(ForwardError::HttpStatus { status, body }) => {
                if status == 401 || status == 403 {
                    eprintln!("context-os-mcp: {}", config::unauthorized_message(status));
                } else {
                    eprintln!("context-os-mcp: HTTP {status} from MCP gateway: {body}");
                }
                if !notification {
                    let message = if status == 401 || status == 403 {
                        config::unauthorized_message(status)
                    } else {
                        format!("local MCP gateway HTTP {status}: {body}")
                    };
                    if let Ok(v) = serde_json::from_str::<Value>(&body) {
                        if v.get("jsonrpc").is_some() {
                            write_stdout_line(&compact_json_line(&v)?)?;
                            continue;
                        }
                    }
                    let err = jsonrpc_error_response(extract_id(&msg), -32000, message);
                    write_stdout_line(&compact_json_line(&err)?)?;
                }
            }
            Err(ForwardError::Other(e)) => {
                eprintln!("context-os-mcp: forward error: {e}");
                if !notification {
                    let err = jsonrpc_error_response(
                        extract_id(&msg),
                        -32603,
                        format!("forward error: {e}"),
                    );
                    write_stdout_line(&compact_json_line(&err)?)?;
                }
            }
        }
    }

    Ok(())
}

enum ForwardError {
    Unreachable(String),
    HttpStatus { status: u16, body: String },
    Other(String),
}

async fn forward_rpc(
    client: &reqwest::Client,
    cfg: &ClientConfig,
    msg: &Value,
) -> Result<Option<String>, ForwardError> {
    let mut req = client
        .post(&cfg.mcp_url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json");
    if let Some(bearer) = cfg.bearer_token() {
        req = req.bearer_auth(bearer);
    }

    let resp = req.json(msg).send().await.map_err(|e| {
        if e.is_connect() {
            ForwardError::Unreachable("connection refused or host unreachable".into())
        } else if e.is_timeout() {
            ForwardError::Unreachable("timeout".into())
        } else {
            ForwardError::Other(e.to_string())
        }
    })?;

    let status = resp.status().as_u16();
    let body = resp
        .text()
        .await
        .map_err(|e| ForwardError::Other(format!("read body: {e}")))?;

    if !(200..300).contains(&status) {
        return Err(ForwardError::HttpStatus { status, body });
    }

    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    match serde_json::from_str::<Value>(trimmed) {
        Ok(v) => compact_json_line(&v)
            .map(Some)
            .map_err(|e| ForwardError::Other(e.to_string())),
        Err(_) => {
            if trimmed.contains('\n') {
                Ok(Some(trimmed.replace('\n', " ")))
            } else {
                Ok(Some(trimmed.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn notification_detection() {
        assert!(is_notification(
            &json!({"jsonrpc":"2.0","method":"notifications/initialized"})
        ));
        assert!(is_notification(
            &json!({"jsonrpc":"2.0","id":null,"method":"notifications/initialized"})
        ));
        assert!(!is_notification(
            &json!({"jsonrpc":"2.0","id":1,"method":"initialize"})
        ));
    }

    #[test]
    fn local_notifications() {
        assert!(handle_local_notification("notifications/initialized"));
        assert!(handle_local_notification("notifications/cancelled"));
        assert!(!handle_local_notification("tools/list"));
    }

    #[test]
    fn compact_has_no_newlines() {
        let v = json!({"a": {"b": [1, 2, 3]}, "s": "hello"});
        let line = compact_json_line(&v).unwrap();
        assert!(!line.contains('\n'));
    }
}
