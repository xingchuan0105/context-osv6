//! Unified desktop IPC error shape + HTTP proxy to local product API.

use super::local_product::product_api_base_url;

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct IpcApiError {
    pub status: u16,
    pub code: String,
    pub message: String,
}

impl IpcApiError {
    pub fn new(status: u16, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(500, "internal_error", message)
    }

    pub fn bad_request(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(400, code, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(404, "not_found", message)
    }

    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self::new(503, "service_unavailable", message)
    }

    pub fn not_implemented(method: &str, path: &str) -> Self {
        Self::new(
            501,
            "not_implemented",
            format!("API call {method} {path} is not yet implemented in desktop mode"),
        )
    }
}

impl From<String> for IpcApiError {
    fn from(message: String) -> Self {
        Self::internal(message)
    }
}

impl From<&str> for IpcApiError {
    fn from(message: &str) -> Self {
        Self::internal(message)
    }
}

impl std::fmt::Display for IpcApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for IpcApiError {}

pub fn not_implemented_api_error(method: &str, path: &str) -> IpcApiError {
    IpcApiError::not_implemented(method, path)
}

fn normalize_path(path: &str) -> String {
    if path.is_empty() {
        "/".into()
    } else if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

/// Proxy REST calls to the local product API (avrag-api on CLIENT_API_PORT).
#[tauri::command]
pub async fn api_call(
    method: String,
    path: String,
    body: Option<serde_json::Value>,
    token: Option<String>,
) -> Result<serde_json::Value, IpcApiError> {
    let base = product_api_base_url();
    let path = normalize_path(&path);
    let url = format!("{base}{path}");
    let method = method.to_uppercase();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| IpcApiError::internal(format!("http client: {e}")))?;

    let mut req = match method.as_str() {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "PATCH" => client.patch(&url),
        "DELETE" => client.delete(&url),
        other => {
            return Err(IpcApiError::bad_request(
                "method_not_allowed",
                format!("Unsupported method {other}"),
            ));
        }
    };

    if let Some(t) = token.as_ref().filter(|t| !t.is_empty()) {
        req = req.bearer_auth(t);
    }
    req = req.header("Accept", "application/json");

    if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
        if let Some(b) = body {
            req = req.json(&b);
        } else {
            req = req.json(&serde_json::json!({}));
        }
    }

    let resp = req.send().await.map_err(|e| {
        if e.is_connect() {
            IpcApiError::service_unavailable(format!(
                "Local product API not reachable at {base}. Start with: bash scripts/desktop-local-product.sh ensure ({e})"
            ))
        } else {
            IpcApiError::internal(format!("request to {url} failed: {e}"))
        }
    })?;

    let status = resp.status().as_u16();
    let text = resp
        .text()
        .await
        .map_err(|e| IpcApiError::internal(format!("read body: {e}")))?;

    if text.trim().is_empty() {
        if (200..300).contains(&status) {
            return Ok(serde_json::json!({ "ok": true, "status": status }));
        }
        return Err(IpcApiError::new(
            status,
            "upstream_error",
            format!("empty body from {method} {path} (HTTP {status})"),
        ));
    }

    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(v) => {
            if (200..300).contains(&status) {
                Ok(v)
            } else {
                // Surface upstream JSON error body when possible.
                let msg = v
                    .get("message")
                    .or_else(|| v.get("error"))
                    .and_then(|m| m.as_str())
                    .unwrap_or(text.as_str());
                Err(IpcApiError::new(status, "upstream_error", msg.to_string()))
            }
        }
        Err(_) => {
            if (200..300).contains(&status) {
                Ok(serde_json::json!({ "raw": text, "status": status }))
            } else {
                Err(IpcApiError::new(status, "upstream_error", text))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_implemented_api_error_maps_to_frontend_contract() {
        let err = not_implemented_api_error("GET", "/api/v1/settings");

        assert_eq!(err.status, 501);
        assert_eq!(err.code, "not_implemented");
        assert!(err.message.contains("GET"));
        assert!(err.message.contains("/api/v1/settings"));
    }

    #[test]
    fn not_implemented_api_error_serializes_structured_fields() {
        let err = not_implemented_api_error("POST", "/api/v1/notebooks");
        let json = serde_json::to_value(&err).expect("serialize ipc api error");

        assert_eq!(json["status"], 501);
        assert_eq!(json["code"], "not_implemented");
        assert_eq!(
            json["message"],
            "API call POST /api/v1/notebooks is not yet implemented in desktop mode"
        );
    }

    #[test]
    fn from_string_uses_internal_code() {
        let err: IpcApiError = "boom".into();
        assert_eq!(err.status, 500);
        assert_eq!(err.code, "internal_error");
        assert_eq!(err.message, "boom");
    }

    #[test]
    fn normalize_path_adds_slash() {
        assert_eq!(normalize_path("health"), "/health");
        assert_eq!(normalize_path("/health"), "/health");
    }
}
