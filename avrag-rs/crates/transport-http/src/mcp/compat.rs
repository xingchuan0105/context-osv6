use axum::{
    body::Bytes,
    extract::{Extension, Path},
    response::{IntoResponse, Response},
};
use serde_json::json;

use crate::middleware::RequestState;

use super::catalog;
use super::gateway;

pub(crate) async fn compat_mcp_sse_handler(
    Path(notebook_id): Path<String>,
    Extension(RequestState(state)): Extension<RequestState>,
) -> Response {
    let request_id = state
        .auth()
        .request_id()
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let payload = json!({
        "jsonrpc": "2.0",
        "method": "ready",
        "params": {
            "request_id": request_id,
            "notebook_id": notebook_id,
            "deprecated": true,
            "tools": catalog::mcp_workspace_query_tools(),
            "rpc_endpoint": format!("/mcp/notebooks/{notebook_id}"),
            "preferred_rpc_endpoint": "/api/v1/mcp",
        }
    });
    (
        axum::http::StatusCode::OK,
        [("content-type", "text/event-stream")],
        format!("event: ready\ndata: {payload}\n\n"),
    )
        .into_response()
}

pub(crate) async fn compat_mcp_jsonrpc_handler(
    Path(notebook_id): Path<String>,
    Extension(RequestState(state)): Extension<RequestState>,
    body: Bytes,
) -> Response {
    gateway::handle_mcp_jsonrpc(&state, Some(notebook_id), body).await
}

pub(crate) async fn compat_mcp_tool_call_handler(
    Path(notebook_id): Path<String>,
    Extension(RequestState(state)): Extension<RequestState>,
    body: Bytes,
) -> Response {
    gateway::legacy_mcp_tool_call_handler(&state, notebook_id, body).await
}
