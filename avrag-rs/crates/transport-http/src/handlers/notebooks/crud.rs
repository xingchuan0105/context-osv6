use avrag_auth::SubjectKind;
use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use common::{AppError, CreateNotebookRequest, UpdateNotebookRequest};
use contracts::notebooks::{NotebookListResponse, NotebookResponse};
use uuid::Uuid;

use super::super::{app_error_response, error_response};
use crate::middleware::RequestState;
use crate::auth_guard::{
    authorize_org_tool, authorize_workspace_notebook_str, org_create_permission,
    org_list_permission, query_permission,
};

#[utoipa::path(
    get,
    path = "/api/v1/notebooks",
    responses(
        (status = 200, description = "List all notebooks", body = NotebookListResponse)
    ),
    tag = "notebooks"
)]
pub(crate) async fn list_notebooks(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Response {
    if let Err(error) = authorize_org_tool(state.auth(), org_list_permission()) {
        return app_error_response(error);
    }
    let notebooks = state.list_notebooks().await;
    (
        StatusCode::OK,
        Json(contracts::notebooks::NotebookListResponse { notebooks }),
    )
        .into_response()
}

#[utoipa::path(
    get,
    path = "/api/v1/notebooks/{id}",
    responses(
        (status = 200, description = "Get a notebook by ID", body = NotebookResponse),
        (status = 404, description = "Notebook not found")
    ),
    params(
        ("id" = String, Path, description = "Notebook ID")
    ),
    tag = "notebooks"
)]
pub(crate) async fn get_notebook(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(id): Path<String>,
) -> Response {
    if let Err(error) = authorize_workspace_notebook_str(state.auth(), query_permission(), &id) {
        return app_error_response(error);
    }
    match state.get_notebook(&id).await {
        Some(nb) => {
            state
                .record_product_event_if_available(
                    analytics::ProductEventName::NotebookOpened,
                    analytics::Surface::Workspace,
                    analytics::ResultTag::Success,
                    None,
                    Uuid::parse_str(&nb.id).ok(),
                    serde_json::json!({
                        "notebook_id": nb.id.clone(),
                    }),
                )
                .await;
            (
                StatusCode::OK,
                Json(contracts::notebooks::NotebookResponse { notebook: nb }),
            )
                .into_response()
        }
        None => error_response(
            StatusCode::NOT_FOUND,
            "not_found",
            &format!("Notebook {id} not found"),
        ),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/notebooks",
    request_body = CreateNotebookRequest,
    responses(
        (status = 201, description = "Notebook created", body = NotebookResponse)
    ),
    tag = "notebooks"
)]
pub(crate) async fn create_notebook(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(req): Json<CreateNotebookRequest>,
) -> Response {
    if let Err(error) = authorize_org_tool(state.auth(), org_create_permission()) {
        return app_error_response(error);
    }
    match state.create_notebook(req).await {
        Ok(nb) => (
            StatusCode::CREATED,
            Json(contracts::notebooks::NotebookResponse { notebook: nb }),
        )
            .into_response(),
        Err(e) => app_error_response(e),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/notebooks/{id}",
    request_body = UpdateNotebookRequest,
    responses(
        (status = 200, description = "Notebook updated", body = NotebookResponse),
        (status = 404, description = "Notebook not found")
    ),
    params(
        ("id" = String, Path, description = "Notebook ID")
    ),
    tag = "notebooks"
)]
pub(crate) async fn update_notebook(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateNotebookRequest>,
) -> Response {
    if matches!(state.auth().subject_kind(), SubjectKind::ApiKey) {
        return app_error_response(AppError::forbidden(
            "api_key_forbidden",
            "API keys cannot modify workspace metadata",
        ));
    }
    match state.update_notebook(&id, req).await {
        Ok(nb) => (
            StatusCode::OK,
            Json(contracts::notebooks::NotebookResponse { notebook: nb }),
        )
            .into_response(),
        Err(e) => app_error_response(e),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/notebooks/{id}",
    responses(
        (status = 200, description = "Notebook deleted"),
        (status = 404, description = "Notebook not found")
    ),
    params(
        ("id" = String, Path, description = "Notebook ID")
    ),
    tag = "notebooks"
)]
pub(crate) async fn delete_notebook(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(id): Path<String>,
) -> Response {
    if matches!(state.auth().subject_kind(), SubjectKind::ApiKey) {
        return app_error_response(AppError::forbidden(
            "api_key_forbidden",
            "API keys cannot modify workspace metadata",
        ));
    }
    match state.delete_notebook(&id).await {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "deleted"})),
        )
            .into_response(),
        Err(e) => app_error_response(e),
    }
}
