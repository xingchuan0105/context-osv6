use app_bootstrap::AppState;
use common::AppError;
use serde_json::Value;

use super::tools;

pub(crate) async fn execute_mcp_tool(
    state: &AppState,
    tool_name: &str,
    arguments: &Value,
) -> Result<Value, AppError> {
    match tool_name {
        "org.create_workspace" => tools::create_workspace(state, arguments).await,
        "org.list_workspaces" => tools::list_workspaces(state, arguments).await,
        "workspace.create_upload" => tools::create_upload(state, arguments).await,
        "workspace.complete_upload" => tools::complete_upload(state, arguments).await,
        "workspace.document_status" => tools::document_status(state, arguments).await,
        "workspace.add_url_source" => tools::add_url_source(state, arguments).await,
        "workspace.list_sources" => tools::list_sources(state, arguments).await,
        "workspace.rag_query" | "workspace.search_query" | "workspace.chat" => {
            tools::execute_query_tool(state, tool_name, arguments).await
        }
        other => Err(AppError::validation(
            "unsupported_tool",
            format!("unsupported MCP tool: {other}"),
        )),
    }
}
