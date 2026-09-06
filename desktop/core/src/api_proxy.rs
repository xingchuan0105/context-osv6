//! 本机产品 API 的 REST 代理与受限上传（原 Tauri `commands/api.rs` 代理部分）。
//!
//! 上传安全边界：只有本机产品 `/uploads/:id` 签名 URL 可接收原始字节
//! （loopback + 端口匹配），WebView fetch 被 CORS 挡住，故由宿主代发。

use base64::{engine::general_purpose::STANDARD, Engine as _};
use std::io::Read;

use crate::host_error::HostError;
use crate::local_product::product_api_base_url;

const LOCAL_API_TIMEOUT_SECS: u64 = 60;
const PUBLISH_EXPORT_TIMEOUT_SECS: u64 = 180;
const UPLOAD_TIMEOUT_SECS: u64 = 300;
const ZSTD_MAX_DECODE: usize = 128 * 1024 * 1024;
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

fn is_publish_export_path(path: &str) -> bool {
    path.contains("/publish/export")
}

fn looks_like_zstd(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[..4] == ZSTD_MAGIC
}

fn encoding_is_zstd(value: &str) -> bool {
    value.split(',').any(|part| {
        part.trim()
            .split(';')
            .next()
            .is_some_and(|token| token.eq_ignore_ascii_case("zstd"))
    })
}

fn decode_response_body(export: bool, encoding: &str, bytes: &[u8]) -> Result<Vec<u8>, HostError> {
    if !export {
        return Ok(bytes.to_vec());
    }
    if encoding_is_zstd(encoding) || looks_like_zstd(bytes) {
        let mut decoder = zstd::stream::read::Decoder::new(bytes)
            .map_err(|err| HostError::internal(format!("zstd: {err}")))?;
        let mut out = Vec::new();
        let mut buf = [0u8; 32 * 1024];
        loop {
            let n = decoder
                .read(&mut buf)
                .map_err(|err| HostError::internal(format!("zstd: {err}")))?;
            if n == 0 {
                break;
            }
            if out.len().saturating_add(n) > ZSTD_MAX_DECODE {
                return Err(HostError::new(
                    413,
                    "publish_export_too_large",
                    "decompressed publish export exceeds 128MiB",
                ));
            }
            out.extend_from_slice(&buf[..n]);
        }
        return Ok(out);
    }
    if bytes.len() > ZSTD_MAX_DECODE {
        return Err(HostError::new(
            413,
            "publish_export_too_large",
            "publish export exceeds 128MiB",
        ));
    }
    Ok(bytes.to_vec())
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
pub async fn api_call(
    method: String,
    path: String,
    body: Option<serde_json::Value>,
    token: Option<String>,
) -> Result<serde_json::Value, HostError> {
    let base = product_api_base_url();
    let path = normalize_path(&path);
    let url = format!("{base}{path}");
    let method = method.to_uppercase();

    let export = is_publish_export_path(&path);
    let timeout_secs = if export {
        PUBLISH_EXPORT_TIMEOUT_SECS
    } else {
        LOCAL_API_TIMEOUT_SECS
    };
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| HostError::internal(format!("http client: {e}")))?;

    let mut req = match method.as_str() {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "PATCH" => client.patch(&url),
        "DELETE" => client.delete(&url),
        other => {
            return Err(HostError::bad_request(
                "method_not_allowed",
                format!("Unsupported method {other}"),
            ));
        }
    };

    if let Some(t) = token.as_ref().filter(|t| !t.is_empty()) {
        req = req.bearer_auth(t);
    }
    req = req.header("Accept", "application/json");
    if export {
        req = req.header("Accept-Encoding", "zstd");
    }

    if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
        if let Some(b) = body {
            req = req.json(&b);
        } else {
            req = req.json(&serde_json::json!({}));
        }
    }

    let resp = req.send().await.map_err(|e| {
        if e.is_connect() {
            HostError::service_unavailable(format!(
                "Local product API not reachable at {base}. Start with: bash scripts/desktop-local-product.sh ensure ({e})"
            ))
        } else {
            HostError::internal(format!("request to {url} failed: {e}"))
        }
    })?;

    let status = resp.status().as_u16();
    let encoding = resp
        .headers()
        .get(reqwest::header::CONTENT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let raw = resp
        .bytes()
        .await
        .map_err(|e| HostError::internal(format!("read body: {e}")))?;
    let decoded = decode_response_body(export, &encoding, &raw)?;
    let text = String::from_utf8_lossy(&decoded);

    if decoded.is_empty() || text.trim().is_empty() {
        if (200..300).contains(&status) {
            return Ok(serde_json::json!({ "ok": true, "status": status }));
        }
        return Err(HostError::new(
            status,
            "upstream_error",
            format!("empty body from {method} {path} (HTTP {status})"),
        ));
    }

    match serde_json::from_slice::<serde_json::Value>(&decoded) {
        Ok(v) => {
            if (200..300).contains(&status) {
                Ok(v)
            } else {
                // Surface upstream JSON error body when possible.
                let msg = v
                    .get("message")
                    .or_else(|| v.get("error"))
                    .and_then(|m| m.as_str())
                    .unwrap_or(text.as_ref());
                Err(HostError::new(status, "upstream_error", msg.to_string()))
            }
        }
        Err(_) => {
            if (200..300).contains(&status) {
                Ok(serde_json::json!({ "raw": text.as_ref(), "status": status }))
            } else {
                Err(HostError::new(status, "upstream_error", text.into_owned()))
            }
        }
    }
}

fn is_loopback_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "::1" | "[::1]")
}

/// Only the local product `/uploads/:id` endpoint may receive raw bytes.
pub fn assert_desktop_upload_url(url: &str, api_base: &str) -> Result<(), HostError> {
    let parsed = url::Url::parse(url).map_err(|e| {
        HostError::bad_request("invalid_upload_url", format!("invalid upload url: {e}"))
    })?;
    let base = url::Url::parse(api_base)
        .map_err(|e| HostError::internal(format!("invalid product API base {api_base}: {e}")))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(HostError::bad_request(
            "invalid_upload_url",
            "upload url must be http(s)",
        ));
    }
    if !is_loopback_host(parsed.host_str().unwrap_or("")) {
        return Err(HostError::bad_request(
            "invalid_upload_url",
            "upload url host must be loopback",
        ));
    }
    if !parsed.path().starts_with("/uploads/") {
        return Err(HostError::bad_request(
            "invalid_upload_url",
            "upload url path must start with /uploads/",
        ));
    }
    if parsed.port_or_known_default() != base.port_or_known_default() {
        return Err(HostError::bad_request(
            "invalid_upload_url",
            "upload url port must match the local product API",
        ));
    }
    Ok(())
}

/// PUT file bytes to a signed local upload URL (WebView fetch is blocked by CORS).
pub async fn upload_bytes(
    url: String,
    content_type: Option<String>,
    body_base64: String,
) -> Result<serde_json::Value, HostError> {
    let base = product_api_base_url();
    assert_desktop_upload_url(&url, &base)?;

    let bytes = STANDARD
        .decode(body_base64.as_bytes())
        .map_err(|e| HostError::bad_request("invalid_body", format!("base64: {e}")))?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(UPLOAD_TIMEOUT_SECS))
        .build()
        .map_err(|e| HostError::internal(format!("http client: {e}")))?;

    let mut req = client.put(&url).body(bytes);
    if let Some(ct) = content_type.as_deref().filter(|s| !s.is_empty()) {
        req = req.header(reqwest::header::CONTENT_TYPE, ct);
    }

    let resp = req.send().await.map_err(|e| {
        if e.is_connect() {
            HostError::service_unavailable(format!(
                "Local product API not reachable at {base} ({e})"
            ))
        } else {
            HostError::internal(format!("upload to {url} failed: {e}"))
        }
    })?;

    let status = resp.status().as_u16();
    let text = resp
        .text()
        .await
        .map_err(|e| HostError::internal(format!("read upload response: {e}")))?;
    if !(200..300).contains(&status) {
        return Err(HostError::new(
            status,
            "upstream_error",
            if text.trim().is_empty() {
                format!("upload failed (HTTP {status})")
            } else {
                text
            },
        ));
    }
    Ok(serde_json::json!({ "ok": true, "status": status }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_zstd_publish_export_roundtrip() {
        let json = br#"{"document_id":"1"}"#;
        let compressed = zstd::encode_all(&json[..], 3).expect("zstd");
        let decoded = decode_response_body(true, "zstd", &compressed).expect("decode");
        assert_eq!(decoded, json);
    }

    #[test]
    fn non_export_skips_zstd_decode() {
        let json = br#"{"ok":true}"#;
        let decoded = decode_response_body(false, "zstd", json).expect("passthrough");
        assert_eq!(decoded, json);
    }

    #[test]
    fn publish_export_path_uses_longer_timeout() {
        assert!(is_publish_export_path(
            "/api/v1/workspaces/abc/publish/export/doc-1"
        ));
        assert!(!is_publish_export_path("/api/v1/workspaces/abc"));
    }

    #[test]
    fn normalize_path_adds_slash() {
        assert_eq!(normalize_path("health"), "/health");
        assert_eq!(normalize_path("/health"), "/health");
    }

    #[test]
    fn desktop_upload_url_allows_local_signed_path() {
        assert!(assert_desktop_upload_url(
            "http://127.0.0.1:18080/uploads/doc-1?expires=1&signature=abc",
            "http://127.0.0.1:18080",
        )
        .is_ok());
    }

    #[test]
    fn desktop_upload_url_rejects_remote_or_wrong_path() {
        assert!(assert_desktop_upload_url(
            "https://evil.example/uploads/doc-1",
            "http://127.0.0.1:18080",
        )
        .is_err());
        assert!(assert_desktop_upload_url(
            "http://127.0.0.1:18080/api/v1/documents",
            "http://127.0.0.1:18080",
        )
        .is_err());
        assert!(assert_desktop_upload_url(
            "http://127.0.0.1:8080/uploads/doc-1",
            "http://127.0.0.1:18080",
        )
        .is_err());
    }
}
