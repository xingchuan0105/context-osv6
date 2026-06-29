use app_bootstrap::AppState;
use common::{AppError, CreateNotebookRequest};
use serde_json::{json, Value};

use crate::auth_guard::{authorize_org_tool, org_create_permission, org_list_permission};
use crate::mcp::catalog;

pub(crate) async fn create_workspace(
    state: &AppState,
    arguments: &Value,
) -> Result<Value, AppError> {
    authorize_org_tool(state.auth(), org_create_permission())?;
    let name = arguments
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if name.is_empty() {
        return Err(AppError::validation("name_required", "name is required"));
    }
    let description = arguments
        .get("description")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    let notebook = state
        .create_notebook(CreateNotebookRequest { name, description })
        .await?;
    Ok(catalog::success_result(
        "org.create_workspace",
        None,
        json!({ "notebook": notebook }),
        vec![
            "Create a workspace API key via POST /api/v1/notebooks/{id}/api-keys (index+query permissions)",
            "workspace.create_upload or workspace.add_url_source",
            "workspace.rag_query after documents are completed",
        ],
    ))
}

pub(crate) async fn list_workspaces(
    state: &AppState,
    _arguments: &Value,
) -> Result<Value, AppError> {
    authorize_org_tool(state.auth(), org_list_permission())?;
    let notebooks = state.list_notebooks().await;
    Ok(catalog::success_result(
        "org.list_workspaces",
        None,
        json!({ "notebooks": notebooks }),
        vec!["org.create_workspace to add another workspace"],
    ))
}
