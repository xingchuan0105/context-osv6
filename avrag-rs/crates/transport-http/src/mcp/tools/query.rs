use app_bootstrap::AppState;
use common::AppError;
use contracts::chat::ChatRequest;
use contracts::documents::DocumentStatus;
use serde_json::{Value, json};

use crate::auth_guard::{authorize_workspace_tool, query_permission, require_workspace_id_arg};

pub(crate) async fn execute_query_tool(
    state: &AppState,
    tool_name: &str,
    arguments: &Value,
) -> Result<Value, AppError> {
    let workspace_id = require_workspace_id_arg(arguments)?;
    authorize_workspace_tool(state.auth(), query_permission(), workspace_id)?;
    let workspace_id_str = workspace_id.to_string();

    let query = arguments
        .get("query")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if query.is_empty() {
        return Err(AppError::validation(
            "query_required",
            "MCP tool call requires arguments.query",
        ));
    }

    let agent_type = match tool_name {
        "workspace.rag_query" | "workspace.chat" => arguments
            .get("agent_type")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("rag")
            .to_string(),
        "workspace.search_query" => "search".to_string(),
        other => {
            return Err(AppError::validation(
                "unsupported_tool",
                format!("unsupported MCP query tool: {other}"),
            ));
        }
    };

    let doc_scope = arguments
        .get("doc_scope")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut req = ChatRequest {
        query,
        workspace_id: Some(workspace_id_str.clone()),
        session_id: None,
        agent_type,
        source_type: None,
        source_token: None,
        doc_scope,
        messages: vec![],
        stream: false,
        debug: false,
        language: None,
        format_hint: None,
    };
    expand_external_workspace_rag_scope(state, &workspace_id_str, &mut req).await?;
    let response = state.agent().chat().execute_chat(req).await?;
    Ok(super::super::catalog::success_result(
        tool_name,
        Some(&workspace_id_str),
        serde_json::to_value(response).unwrap_or_else(|_| json!({})),
        vec![],
    ))
}

pub(crate) async fn expand_external_workspace_rag_scope(
    state: &AppState,
    workspace_id: &str,
    req: &mut ChatRequest,
) -> Result<(), AppError> {
    if req.agent_type != "rag" || !req.doc_scope.is_empty() {
        return Ok(());
    }

    state.docs()
        .get_workspace(workspace_id)
        .await
        .ok_or_else(|| AppError::not_found("workspace_not_found", "workspace not found"))?;
    let doc_scope = state.docs()
        .list_documents(Some(workspace_id), None)
        .await
        .into_iter()
        .filter(|document| matches!(document.status, DocumentStatus::Completed))
        .map(|document| document.id)
        .collect::<Vec<_>>();
    if doc_scope.is_empty() {
        return Err(AppError::validation(
            "docscope_required",
            "No ready documents are available in this notebook for RAG.",
        ));
    }

    req.doc_scope = doc_scope;
    Ok(())
}
