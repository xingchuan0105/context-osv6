//! Route handler implementations for the transport-http crate.

use axum::{
    Json,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use common::AppError;

mod chat;
mod documents;
mod notebooks;

pub(crate) use chat::*;
pub(crate) use documents::*;
pub(crate) use notebooks::*;

// ---------------------------------------------------------------------------
// Error helpers
// ---------------------------------------------------------------------------

/// Convert an [`AppError`] into an HTTP response with a typed JSON body.
pub(crate) fn app_error_response(e: AppError) -> Response {
    let status = StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut body = serde_json::json!({
        "error": e.code(),
        "message": e.message(),
    });
    let retry_after = e.retry_after_secs();
    if let Some(secs) = retry_after {
        body["retry_after_secs"] = serde_json::json!(secs);
    }
    let mut response = (status, Json(body)).into_response();
    if status == StatusCode::TOO_MANY_REQUESTS {
        if let Some(secs) = retry_after {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from(secs as u64));
        }
    }
    response
}

/// Return a JSON error response.
pub(crate) fn error_response(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({
            "error": code,
            "message": message,
        })),
    )
        .into_response()
}
