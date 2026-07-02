use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};

pub(crate) fn jsonrpc_response(id: Option<Value>, result: Value) -> Response {
    (
        StatusCode::OK,
        Json(json!({
            "jsonrpc": "2.0",
            "id": id.unwrap_or(Value::Null),
            "result": result,
        })),
    )
        .into_response()
}

pub(crate) fn jsonrpc_error(id: Option<Value>, code: i32, message: impl Into<String>) -> Response {
    (
        StatusCode::OK,
        Json(json!({
            "jsonrpc": "2.0",
            "id": id.unwrap_or(Value::Null),
            "error": {
                "code": code,
                "message": message.into(),
            }
        })),
    )
        .into_response()
}

pub(crate) fn tool_call_success(id: Option<Value>, structured: Value) -> Response {
    jsonrpc_response(
        id,
        json!({
            "content": [{
                "type": "text",
                "text": structured.to_string(),
            }],
            "structuredContent": structured,
        }),
    )
}

pub(crate) fn invalid_json_response(error: serde_json::Error) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({
            "error": "invalid_json",
            "message": format!("invalid MCP payload: {error}"),
        })),
    )
        .into_response()
}

pub(crate) fn method_required_response() -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({
            "error": "method_required",
            "message": "MCP JSON-RPC request requires method",
        })),
    )
        .into_response()
}
