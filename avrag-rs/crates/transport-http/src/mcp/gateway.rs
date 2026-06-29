use app_bootstrap::AppState;
use axum::{
    body::Bytes,
    extract::Extension,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::handlers;
use crate::RequestState;

use super::catalog;
use super::dispatch;
use super::jsonrpc;

pub(crate) async fn unified_mcp_jsonrpc_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    body: Bytes,
) -> Response {
    handle_mcp_jsonrpc(&state, None, body).await
}

pub(crate) async fn unified_mcp_sse_handler(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Response {
    let request_id = state
        .auth()
        .request_id()
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let payload = json!({
        "jsonrpc": "2.0",
        "method": "ready",
        "params": {
            "request_id": request_id,
            "tools": catalog::mcp_all_tools(),
            "rpc_endpoint": "/api/v1/mcp",
        }
    });
    (
        StatusCode::OK,
        [("content-type", "text/event-stream")],
        format!("event: ready\ndata: {payload}\n\n"),
    )
        .into_response()
}

pub(crate) async fn handle_mcp_jsonrpc(
    state: &AppState,
    default_notebook_id: Option<String>,
    body: Bytes,
) -> Response {
    let request_json: Value = match serde_json::from_slice(body.as_ref()) {
        Ok(value) => value,
        Err(error) => return jsonrpc::invalid_json_response(error),
    };

    let id = request_json.get("id").cloned();
    let method = request_json
        .get("method")
        .and_then(|value| value.as_str())
        .unwrap_or_default();

    match method {
        "initialize" => jsonrpc::jsonrpc_response(
            id,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "context-os",
                    "version": "0.1.0"
                }
            }),
        ),
        "tools/list" => jsonrpc::jsonrpc_response(id, json!({ "tools": catalog::mcp_all_tools() })),
        "tools/call" => {
            let tool_name = request_json
                .pointer("/params/name")
                .and_then(|value| value.as_str())
                .unwrap_or("notebook.chat");
            let mut arguments = request_json
                .pointer("/params/arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            inject_default_notebook_id(default_notebook_id.as_deref(), &mut arguments);
            match dispatch::execute_mcp_tool(state, tool_name, &arguments).await {
                Ok(result) => jsonrpc::tool_call_success(id, result),
                Err(error) => handlers::mcp_tool_call_error_response(
                    id,
                    error,
                    catalog::operation_guide_mode_for_tool(tool_name),
                ),
            }
        }
        "" => jsonrpc::method_required_response(),
        _ => jsonrpc::jsonrpc_error(id, -32601, format!("method not found: {method}")),
    }
}

pub(crate) async fn legacy_mcp_tool_call_handler(
    state: &AppState,
    notebook_id: String,
    body: Bytes,
) -> Response {
    let request_json: Value = match serde_json::from_slice(body.as_ref()) {
        Ok(value) => value,
        Err(error) => return jsonrpc::invalid_json_response(error),
    };

    let tool_name = request_json
        .pointer("/params/name")
        .and_then(|value| value.as_str())
        .unwrap_or("notebook.chat");
    let mut arguments = request_json
        .pointer("/params/arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if arguments.get("query").is_none()
        && let Some(query) = request_json.get("query")
        && let Some(obj) = arguments.as_object_mut()
    {
        obj.insert("query".to_string(), query.clone());
    }
    inject_default_notebook_id(Some(notebook_id.as_str()), &mut arguments);

    match dispatch::execute_mcp_tool(state, tool_name, &arguments).await {
        Ok(mut result) => {
            if let Some(obj) = result.as_object_mut() {
                obj.insert("deprecated".to_string(), json!(true));
            }
            if request_json.get("id").is_some() {
                (
                    StatusCode::OK,
                    axum::Json(json!({
                        "jsonrpc": "2.0",
                        "id": request_json.get("id").cloned().unwrap_or(Value::Null),
                        "result": result,
                    })),
                )
                    .into_response()
            } else {
                (StatusCode::OK, axum::Json(result)).into_response()
            }
        }
        Err(error) => handlers::mcp_tool_call_error_response(
            request_json.get("id").cloned(),
            error,
            catalog::operation_guide_mode_for_tool(tool_name),
        ),
    }
}

fn inject_default_notebook_id(notebook_id: Option<&str>, arguments: &mut Value) {
    let Some(notebook_id) = notebook_id else {
        return;
    };
    let Some(obj) = arguments.as_object_mut() else {
        return;
    };
    if obj
        .get("notebook_id")
        .and_then(|value| value.as_str())
        .is_none_or(|value| value.trim().is_empty())
    {
        obj.insert("notebook_id".to_string(), json!(notebook_id));
    }
}
