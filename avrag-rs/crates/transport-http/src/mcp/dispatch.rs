use app_bootstrap::AppState;
use common::AppError;
use serde_json::Value;

use super::catalog;
use super::tools;

fn require_subtex_enabled(tool_name: &str) -> Result<(), AppError> {
    if catalog::subtex_tools_enabled() {
        Ok(())
    } else {
        Err(AppError::validation(
            "unsupported_tool",
            format!("unsupported MCP tool: {tool_name}"),
        ))
    }
}

pub(crate) async fn execute_mcp_tool(
    state: &AppState,
    tool_name: &str,
    arguments: &Value,
) -> Result<Value, AppError> {
    match tool_name {
        "account.create_workspace" => tools::create_workspace(state, arguments).await,
        "account.list_workspaces" => tools::list_workspaces(state, arguments).await,
        "workspace.create_upload" => tools::create_upload(state, arguments).await,
        "workspace.complete_upload" => tools::complete_upload(state, arguments).await,
        "workspace.document_status" => tools::document_status(state, arguments).await,
        "workspace.add_url_source" => tools::add_url_source(state, arguments).await,
        "workspace.list_sources" => tools::list_sources(state, arguments).await,
        "workspace.rag_query" | "workspace.search_query" | "workspace.chat" => {
            tools::execute_query_tool(state, tool_name, arguments).await
        }
        "workspace.share_create_link" => tools::share_create_link(state, arguments).await,
        "workspace.share_get_settings" => tools::share_get_settings(state, arguments).await,
        "workspace.share_update_settings" => tools::share_update_settings(state, arguments).await,
        "workspace.share_revoke_link" => tools::share_revoke_link(state, arguments).await,
        "workspace.share_invite_member" => tools::share_invite_member(state, arguments).await,
        "account.share_quota" => tools::share_quota(state, arguments).await,
        "subtex.init"
        | "subtex.status"
        | "subtex.convention_draft"
        | "subtex.correction_draft"
        | "subtex.search"
        | "subtex.outline"
        | "subtex.transcribe"
        | "subtex.inbox"
        | "subtex.inbox_decide" => {
            require_subtex_enabled(tool_name)?;
            match tool_name {
                "subtex.init" => tools::subtex_init(state, arguments).await,
                "subtex.status" => tools::subtex_status(state, arguments).await,
                "subtex.convention_draft" => tools::subtex_convention_draft(state, arguments).await,
                "subtex.correction_draft" => {
                    tools::subtex_correction_draft(state, arguments).await
                }
                "subtex.search" => tools::subtex_search(state, arguments).await,
                "subtex.outline" => tools::subtex_outline(state, arguments).await,
                "subtex.transcribe" => tools::subtex_transcribe(state, arguments).await,
                "subtex.inbox" => tools::subtex_inbox(state, arguments).await,
                "subtex.inbox_decide" => tools::subtex_inbox_decide(state, arguments).await,
                _ => unreachable!("subtex tool name already matched"),
            }
        }
        other => Err(AppError::validation(
            "unsupported_tool",
            format!("unsupported MCP tool: {other}"),
        )),
    }
}
