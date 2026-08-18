//! Workspace publish HTTP handlers (ADR-0010 B3b).

use app_core::{CreatePublishSessionRequest, PublishPartPayload};
use axum::{
    Json,
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use uuid::Uuid;

use super::super::app_error_response;
use crate::auth_guard::require_user_session;
use crate::middleware::RequestState;

fn require_session(state: &app_bootstrap::AppState) -> Result<(), Response> {
    require_user_session(
        state.auth(),
        "publish requires a signed-in user session",
    )
    .map_err(app_error_response)
}

#[derive(Debug, Deserialize)]
pub(crate) struct PublishStatusQuery {
    pub local_workspace_id: Uuid,
}

pub(crate) async fn create_publish_session_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(body): Json<CreatePublishSessionRequest>,
) -> Response {
    if let Err(error) = require_session(&state) {
        return error;
    }
    match state.workspace().create_publish_session(body).await {
        Ok(payload) => (StatusCode::OK, Json(payload)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn put_publish_part_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((upload_id, part_n)): Path<(Uuid, u32)>,
    Json(body): Json<PublishPartPayload>,
) -> Response {
    if let Err(error) = require_session(&state) {
        return error;
    }
    match state.workspace().put_publish_part(upload_id, part_n, body).await {
        Ok(()) => (StatusCode::NO_CONTENT,).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn commit_publish_session_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(upload_id): Path<Uuid>,
) -> Response {
    if let Err(error) = require_session(&state) {
        return error;
    }
    match state.workspace().commit_publish_session(upload_id).await {
        Ok(payload) => (StatusCode::OK, Json(payload)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn get_publish_status_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Query(query): Query<PublishStatusQuery>,
) -> Response {
    if let Err(error) = require_session(&state) {
        return error;
    }
    match state
        .workspace()
        .get_publish_status(query.local_workspace_id)
        .await
    {
        Ok(payload) => (StatusCode::OK, Json(payload)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn export_publish_list_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(workspace_id): Path<Uuid>,
) -> Response {
    if let Err(error) = require_session(&state) {
        return error;
    }
    match state.workspace().export_publish_list(workspace_id).await {
        Ok(payload) => (StatusCode::OK, Json(payload)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn export_publish_document_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((workspace_id, document_id)): Path<(Uuid, Uuid)>,
) -> Response {
    if let Err(error) = require_session(&state) {
        return error;
    }
    match state
        .workspace()
        .export_publish_document(workspace_id, document_id)
        .await
    {
        Ok(payload) => (StatusCode::OK, Json(payload)).into_response(),
        Err(error) => app_error_response(error),
    }
}
