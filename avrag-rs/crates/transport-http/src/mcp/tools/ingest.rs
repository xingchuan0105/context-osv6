use app_bootstrap::AppState;
use common::{AddUrlSourceRequest, AppError, CreateDocumentRequest};
use contracts::documents::DocumentStatus;
use serde_json::{json, Value};

use crate::auth_guard::{
    authorize_workspace_index_or_query, authorize_workspace_tool, ensure_document_in_notebook,
    index_permission, query_permission, require_notebook_id_arg,
};
use crate::mcp::catalog;

pub(crate) async fn create_upload(
    state: &AppState,
    arguments: &Value,
) -> Result<Value, AppError> {
    let notebook_id = require_notebook_id_arg(arguments)?;
    authorize_workspace_tool(state.auth(), index_permission(), notebook_id)?;
    let notebook_id_str = notebook_id.to_string();

    let filename = arguments
        .get("filename")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    let mime_type = arguments
        .get("mime_type")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    let file_size = arguments
        .get("file_size")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    if filename.is_empty() || mime_type.is_empty() || file_size == 0 {
        return Err(AppError::validation(
            "invalid_upload_request",
            "filename, mime_type, and file_size are required",
        ));
    }

    let upload = state
        .create_document_upload(
            &notebook_id_str,
            CreateDocumentRequest {
                filename,
                mime_type,
                file_size,
            },
        )
        .await?;

    Ok(catalog::success_result(
        "workspace.create_upload",
        Some(&notebook_id_str),
        json!(upload),
        vec![
            "HTTP PUT file bytes to data.upload_url",
            "workspace.complete_upload with document_id",
            "workspace.document_status until completed",
        ],
    ))
}

pub(crate) async fn complete_upload(
    state: &AppState,
    arguments: &Value,
) -> Result<Value, AppError> {
    let notebook_id = require_notebook_id_arg(arguments)?;
    authorize_workspace_tool(state.auth(), index_permission(), notebook_id)?;
    let notebook_id_str = notebook_id.to_string();
    let document_id = arguments
        .get("document_id")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if document_id.is_empty() {
        return Err(AppError::validation(
            "document_id_required",
            "document_id is required",
        ));
    }
    ensure_document_in_notebook(state, &document_id, &notebook_id_str).await?;

    let result = state.complete_document_upload(&document_id).await?;
    Ok(catalog::success_result(
        "workspace.complete_upload",
        Some(&notebook_id_str),
        json!(result),
        vec!["workspace.document_status until status is completed"],
    ))
}

pub(crate) async fn document_status(
    state: &AppState,
    arguments: &Value,
) -> Result<Value, AppError> {
    let notebook_id = require_notebook_id_arg(arguments)?;
    authorize_workspace_index_or_query(state.auth(), notebook_id)?;
    let notebook_id_str = notebook_id.to_string();
    let document_id = arguments
        .get("document_id")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if document_id.is_empty() {
        return Err(AppError::validation(
            "document_id_required",
            "document_id is required",
        ));
    }

    let document = state
        .list_documents(None, Some(&document_id))
        .await
        .into_iter()
        .next()
        .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
    if document.notebook_id != notebook_id_str {
        return Err(AppError::forbidden(
            "document_notebook_mismatch",
            "document does not belong to the requested workspace",
        ));
    }

    Ok(catalog::success_result(
        "workspace.document_status",
        Some(&notebook_id_str),
        json!({
            "document_id": document.id,
            "status": document.status.as_str(),
        }),
        if document.status == DocumentStatus::Completed {
            vec!["workspace.rag_query"]
        } else {
            vec!["poll workspace.document_status again"]
        },
    ))
}

pub(crate) async fn add_url_source(
    state: &AppState,
    arguments: &Value,
) -> Result<Value, AppError> {
    let notebook_id = require_notebook_id_arg(arguments)?;
    authorize_workspace_tool(state.auth(), index_permission(), notebook_id)?;
    let notebook_id_str = notebook_id.to_string();
    let url = arguments
        .get("url")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if url.is_empty() {
        return Err(AppError::validation("url_required", "url is required"));
    }

    let source = state
        .add_url_source(&notebook_id_str, AddUrlSourceRequest { url })
        .await?;
    Ok(catalog::success_result(
        "workspace.add_url_source",
        Some(&notebook_id_str),
        json!(source),
        vec!["workspace.document_status until completed", "workspace.rag_query"],
    ))
}

pub(crate) async fn list_sources(
    state: &AppState,
    arguments: &Value,
) -> Result<Value, AppError> {
    let notebook_id = require_notebook_id_arg(arguments)?;
    authorize_workspace_tool(state.auth(), query_permission(), notebook_id)?;
    let notebook_id_str = notebook_id.to_string();
    let sources = state.list_sources(Some(&notebook_id_str)).await;
    Ok(catalog::success_result(
        "workspace.list_sources",
        Some(&notebook_id_str),
        json!({ "sources": sources }),
        vec!["workspace.rag_query"],
    ))
}
