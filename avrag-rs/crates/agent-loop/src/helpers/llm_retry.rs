//! Classify LLM/upstream failures for bounded synthesis retries.
//!
//! LLM calls are not idempotent; retries are **delivery-idempotent** (one final
//! answer per turn). Only transient transport / gateway failures are retryable.

use avrag_llm::LlmError;
use common::AppError;

/// True when the error is a user/host cancellation — never retry.
pub fn is_cancellation_error(err: &anyhow::Error) -> bool {
    for cause in err.chain() {
        if let Some(llm) = cause.downcast_ref::<LlmError>() {
            if matches!(llm, LlmError::Cancelled) {
                return true;
            }
        }
    }
    let s = format!("{err:#}").to_ascii_lowercase();
    s.contains("request cancelled") || s.contains("cancelled during")
}

/// Transient upstream / stream failures worth a non-stream fallback.
pub fn is_retryable_upstream_error(err: &anyhow::Error) -> bool {
    if is_cancellation_error(err) {
        return false;
    }
    for cause in err.chain() {
        if let Some(llm) = cause.downcast_ref::<LlmError>() {
            return llm_error_is_retryable(llm);
        }
    }
    // anyhow-wrapped messages from our own context strings
    let s = format!("{err:#}").to_ascii_lowercase();
    s.contains("stream chunk")
        || s.contains("connection")
        || s.contains("timed out")
        || s.contains("timeout")
        || s.contains("broken pipe")
        || s.contains("connection reset")
        || s.contains("empty stream")
        || s.contains("connection closed")
        || s.contains("error sending request")
        || s.contains("tcp")
        || s.contains("502")
        || s.contains("503")
        || s.contains("504")
        || s.contains("429")
}

fn llm_error_is_retryable(err: &LlmError) -> bool {
    match err {
        LlmError::Cancelled | LlmError::Config(_) => false,
        LlmError::EmptyStream => true,
        LlmError::Parse(_) => true, // mid-stream JSON/SSE glitches
        LlmError::Protocol(msg) => {
            let m = msg.to_ascii_lowercase();
            m.contains("without content") || m.contains("empty") || m.contains("incomplete")
        }
        LlmError::Api { status, .. } => matches!(*status, 408 | 425 | 429 | 500 | 502 | 503 | 504),
        LlmError::Http(e) => {
            e.is_timeout()
                || e.is_connect()
                || e.is_request()
                || e.is_body()
                || e.is_decode()
                || e.status()
                    .map(|s| matches!(s.as_u16(), 408 | 425 | 429 | 500 | 502 | 503 | 504))
                    .unwrap_or(false)
        }
        LlmError::Other(inner) => is_retryable_upstream_error(inner),
    }
}

/// Map an LLM failure to a stable product error after retries are exhausted.
pub fn map_llm_error_to_app_error(context: &str, err: anyhow::Error) -> AppError {
    if is_cancellation_error(&err) {
        return AppError::internal(format!("{context}: cancelled"));
    }
    if is_retryable_upstream_error(&err) {
        return AppError::upstream_unavailable(format!("{context}: {err}"));
    }
    // 4xx-class API errors stay as internal_code so code is visible without
    // claiming "service unavailable".
    for cause in err.chain() {
        if let Some(LlmError::Api { status, body }) = cause.downcast_ref::<LlmError>() {
            if *status == 401 || *status == 403 {
                return AppError::internal_code(
                    "upstream_auth_failed",
                    format!("{context}: HTTP {status}: {body}"),
                );
            }
            if (400..500).contains(status) {
                return AppError::internal_code(
                    "upstream_request_rejected",
                    format!("{context}: HTTP {status}: {body}"),
                );
            }
        }
    }
    AppError::internal(format!("{context}: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;

    #[test]
    fn stream_chunk_message_is_retryable() {
        let err = anyhow!("Failed to read chat completion stream chunk").context("stream");
        assert!(is_retryable_upstream_error(&err));
        assert!(!is_cancellation_error(&err));
    }

    #[test]
    fn api_503_is_retryable() {
        let err = anyhow::Error::new(LlmError::Api {
            status: 503,
            body: "unavailable".into(),
        });
        assert!(is_retryable_upstream_error(&err));
        let app = map_llm_error_to_app_error("synthesis stream failed", err);
        assert_eq!(app.code(), "upstream_unavailable");
        assert_eq!(app.http_status(), 503);
        assert!(app.is_retryable());
    }

    #[test]
    fn api_400_is_not_retryable() {
        let err = anyhow::Error::new(LlmError::Api {
            status: 400,
            body: "bad request".into(),
        });
        assert!(!is_retryable_upstream_error(&err));
        let app = map_llm_error_to_app_error("synthesis", err);
        assert_eq!(app.code(), "upstream_request_rejected");
        assert!(!app.is_retryable());
    }

    #[test]
    fn cancelled_is_not_retryable() {
        let err = anyhow::Error::new(LlmError::Cancelled);
        assert!(!is_retryable_upstream_error(&err));
        assert!(is_cancellation_error(&err));
    }

    #[test]
    fn empty_stream_is_retryable() {
        let err = anyhow::Error::new(LlmError::EmptyStream);
        assert!(is_retryable_upstream_error(&err));
    }
}
