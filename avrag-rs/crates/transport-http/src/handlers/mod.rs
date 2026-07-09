//! Route handler implementations for the transport-http crate.

use axum::{
    Json,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use common::AppError;

mod chat;
mod documents;
mod workspaces;

pub(crate) use chat::*;
pub(crate) use documents::*;
pub(crate) use workspaces::*;

// ---------------------------------------------------------------------------
// Error helpers
// ---------------------------------------------------------------------------

/// Convert an [`AppError`] into an HTTP response with a typed JSON body.
pub(crate) fn app_error_response(e: AppError) -> Response {
    app_error_response_for_agent(e, None)
}

pub(crate) fn app_error_response_for_agent(e: AppError, agent_type: Option<&str>) -> Response {
    let status = StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut body = serde_json::json!({
        "error": e.code(),
        "message": e.message(),
    });
    if let Some(agent_type) = agent_type
        && let Some(guide) = app_chat::load_invoke_operation_guide(agent_type)
    {
        body["agent_operation_guide"] =
            serde_json::to_value(guide).unwrap_or(serde_json::Value::Null);
    }
    let retry_after = e.retry_after_secs();
    if let Some(secs) = retry_after {
        body["retry_after_secs"] = serde_json::json!(secs);
    }
    let mut response = (status, Json(body)).into_response();
    if status == StatusCode::TOO_MANY_REQUESTS
        && let Some(secs) = retry_after
    {
        response
            .headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from(secs));
    }
    response
}

pub(crate) fn operation_guide_agent_type(agent_type: &str) -> Option<&str> {
    match agent_type {
        "rag" | "search" | "index" | "workspace.create" => Some(agent_type),
        _ => None,
    }
}

pub(crate) fn mcp_tool_call_error_response(
    id: Option<serde_json::Value>,
    error: AppError,
    guide_mode: Option<&str>,
) -> Response {
    let rpc_code = match error.http_status() {
        400 => -32602,
        401 => -32001,
        403 => -32003,
        404 => -32004,
        429 => -32029,
        _ => -32603,
    };
    let mut data = serde_json::json!({
        "error": error.code(),
        "message": error.message(),
    });
    if let Some(mode) = guide_mode
        && let Some(guide) = app_chat::load_invoke_operation_guide(mode)
    {
        data["agent_operation_guide"] =
            serde_json::to_value(guide).unwrap_or(serde_json::Value::Null);
    }
    if let Some(secs) = error.retry_after_secs() {
        data["retry_after_secs"] = serde_json::json!(secs);
    }
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": id.unwrap_or(serde_json::Value::Null),
            "error": {
                "code": rpc_code,
                "message": error.message(),
                "data": data,
            }
        })),
    )
        .into_response()
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
