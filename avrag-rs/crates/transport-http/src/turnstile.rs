//! Cloudflare Turnstile verification for anonymous share chat (ADR-0010 §9).
//!
//! When `TURNSTILE_SECRET_KEY` is set, anonymous share chat requests must present
//! a valid Turnstile token (`cf-turnstile-response` header or JSON field).
//! When the secret is empty, verification is skipped (local/dev).

use common::AppError;

/// Siteverify endpoint (Cloudflare free Turnstile).
const SITEVERIFY_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";

pub fn turnstile_secret() -> Option<String> {
    std::env::var("TURNSTILE_SECRET_KEY")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn extract_turnstile_token(
    headers: &axum::http::HeaderMap,
    body: &serde_json::Value,
) -> Option<String> {
    headers
        .get("cf-turnstile-response")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            body.get("cf_turnstile_response")
                .or_else(|| body.get("turnstile_token"))
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
}

/// Verify token when secret configured. No-op (Ok) when Turnstile is not configured.
pub async fn ensure_turnstile_if_required(
    headers: &axum::http::HeaderMap,
    body: &serde_json::Value,
    remote_ip: Option<&str>,
) -> Result<(), AppError> {
    let Some(secret) = turnstile_secret() else {
        return Ok(());
    };
    let Some(token) = extract_turnstile_token(headers, body) else {
        return Err(AppError::validation(
            "turnstile_required",
            "Anonymous share chat requires a Cloudflare Turnstile challenge token.",
        ));
    };
    verify_turnstile(&secret, &token, remote_ip).await
}

async fn verify_turnstile(
    secret: &str,
    token: &str,
    remote_ip: Option<&str>,
) -> Result<(), AppError> {
    // Test hook: accept fixed token when secret is "test".
    if secret == "test" {
        if token == "test-ok" {
            return Ok(());
        }
        return Err(AppError::validation(
            "turnstile_failed",
            "Turnstile test token rejected",
        ));
    }

    // Shared client with a hard timeout — per-request Client::new() has no
    // connect pool reuse and no ceiling on a hung siteverify call.
    static CLIENT: std::sync::LazyLock<reqwest::Client> = std::sync::LazyLock::new(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    });
    let client = &*CLIENT;
    let mut form = vec![
        ("secret", secret.to_string()),
        ("response", token.to_string()),
    ];
    if let Some(ip) = remote_ip.filter(|s| !s.is_empty()) {
        form.push(("remoteip", ip.to_string()));
    }
    let resp = client
        .post(SITEVERIFY_URL)
        .form(&form)
        .send()
        .await
        .map_err(|e| AppError::internal(format!("turnstile siteverify request failed: {e}")))?;
    let status = resp.status();
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::internal(format!("turnstile siteverify decode failed: {e}")))?;
    let success = body
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if success {
        return Ok(());
    }
    tracing::warn!(%status, body = %body, "turnstile verification failed");
    Err(AppError::validation(
        "turnstile_failed",
        "Cloudflare Turnstile verification failed. Retry the challenge.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn extract_from_header_or_body() {
        let mut headers = HeaderMap::new();
        headers.insert("cf-turnstile-response", "hdr-token".parse().unwrap());
        let body = serde_json::json!({});
        assert_eq!(
            extract_turnstile_token(&headers, &body).as_deref(),
            Some("hdr-token")
        );
        let body2 = serde_json::json!({ "turnstile_token": "body-token" });
        assert_eq!(
            extract_turnstile_token(&HeaderMap::new(), &body2).as_deref(),
            Some("body-token")
        );
    }

    #[tokio::test]
    async fn test_secret_accepts_test_ok() {
        // Call verify path directly to avoid mutating process env in parallel tests.
        verify_turnstile("test", "test-ok", None)
            .await
            .expect("test-ok");
        assert!(verify_turnstile("test", "bad", None).await.is_err());
    }
}
