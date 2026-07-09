use contracts::auth_runtime::SubjectKind;
use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use common::{AppError, CreateWorkspaceRequest, UpdateWorkspaceRequest};
use contracts::workspaces::{WorkspaceListResponse, WorkspaceResponse};
use uuid::Uuid;

use super::super::{app_error_response, error_response};
use crate::middleware::RequestState;
use crate::auth_guard::{
    authorize_org_tool, authorize_workspace_notebook_str, org_create_permission,
    org_list_permission, query_permission,
};

#[utoipa::path(
    get,
    path = "/api/v1/workspaces",
    responses(
        (status = 200, description = "List all notebooks", body = WorkspaceListResponse)
    ),
    tag = "workspaces"
)]
pub(crate) async fn list_workspaces(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Response {
    if let Err(error) = authorize_org_tool(state.auth(), org_list_permission()) {
        return app_error_response(error);
    }
    let workspaces = state.docs().list_workspaces().await;
    (
        StatusCode::OK,
        Json(contracts::workspaces::WorkspaceListResponse { workspaces }),
    )
        .into_response()
}

#[utoipa::path(
    get,
    path = "/api/v1/workspaces/{id}",
    responses(
        (status = 200, description = "Get a notebook by ID", body = WorkspaceResponse),
        (status = 404, description = "Workspace not found")
    ),
    params(
        ("id" = String, Path, description = "Workspace ID")
    ),
    tag = "workspaces"
)]
pub(crate) async fn get_workspace(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(id): Path<String>,
) -> Response {
    if let Err(error) = authorize_workspace_notebook_str(state.auth(), query_permission(), &id) {
        return app_error_response(error);
    }
    match state.docs().get_workspace(&id).await {
        Some(nb) => {
            state
                .record_product_event_if_available(
                    analytics::ProductEventName::WorkspaceOpened,
                    analytics::Surface::Workspace,
                    analytics::ResultTag::Success,
                    None,
                    Uuid::parse_str(&nb.id).ok(),
                    serde_json::json!({
                        "workspace_id": nb.id.clone(),
                    }),
                )
                .await;
            (
                StatusCode::OK,
                Json(contracts::workspaces::WorkspaceResponse { workspace: nb }),
            )
                .into_response()
        }
        None => error_response(
            StatusCode::NOT_FOUND,
            "not_found",
            &format!("Workspace {id} not found"),
        ),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/workspaces",
    request_body = CreateWorkspaceRequest,
    responses(
        (status = 201, description = "Workspace created", body = WorkspaceResponse)
    ),
    tag = "workspaces"
)]
pub(crate) async fn create_workspace(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(req): Json<CreateWorkspaceRequest>,
) -> Response {
    if let Err(error) = authorize_org_tool(state.auth(), org_create_permission()) {
        return app_error_response(error);
    }
    match state.docs().create_workspace(req).await {
        Ok(nb) => (
            StatusCode::CREATED,
            Json(contracts::workspaces::WorkspaceResponse { workspace: nb }),
        )
            .into_response(),
        Err(e) => app_error_response(e),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/workspaces/{id}",
    request_body = UpdateWorkspaceRequest,
    responses(
        (status = 200, description = "Workspace updated", body = WorkspaceResponse),
        (status = 404, description = "Workspace not found")
    ),
    params(
        ("id" = String, Path, description = "Workspace ID")
    ),
    tag = "workspaces"
)]
pub(crate) async fn update_workspace(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateWorkspaceRequest>,
) -> Response {
    if matches!(state.auth().subject_kind(), SubjectKind::ApiKey) {
        return app_error_response(AppError::forbidden(
            "api_key_forbidden",
            "API keys cannot modify workspace metadata",
        ));
    }
    match state.docs().update_workspace(&id, req).await {
        Ok(nb) => (
            StatusCode::OK,
            Json(contracts::workspaces::WorkspaceResponse { workspace: nb }),
        )
            .into_response(),
        Err(e) => app_error_response(e),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/workspaces/{id}",
    responses(
        (status = 200, description = "Workspace deleted"),
        (status = 404, description = "Workspace not found")
    ),
    params(
        ("id" = String, Path, description = "Workspace ID")
    ),
    tag = "workspaces"
)]
pub(crate) async fn delete_workspace(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(id): Path<String>,
) -> Response {
    if matches!(state.auth().subject_kind(), SubjectKind::ApiKey) {
        return app_error_response(AppError::forbidden(
            "api_key_forbidden",
            "API keys cannot modify workspace metadata",
        ));
    }
    match state.docs().delete_workspace(&id).await {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "deleted"})),
        )
            .into_response(),
        Err(e) => app_error_response(e),
    }
}
