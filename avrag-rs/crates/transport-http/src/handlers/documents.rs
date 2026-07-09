use axum::{
    Json,
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use common::{AddUrlSourceRequest, CreateDocumentRequest, UpdateDocumentRequest};

use super::chat::ChatSessionsQuery;
use super::{app_error_response, error_response};
use crate::middleware::RequestState;
use crate::auth_guard::{
    authorize_document_access, authorize_document_access_index_or_query,
    authorize_workspace_notebook_str, authorize_workspace_query_optional_notebook,
    index_permission,
};

pub(crate) async fn list_documents_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Query(params): Query<ChatSessionsQuery>,
) -> Response {
    if let Err(error) =
        authorize_workspace_query_optional_notebook(state.auth(), params.notebook_id.as_deref())
    {
        return app_error_response(error);
    }
    let documents = state
        .list_documents(params.notebook_id.as_deref(), None)
        .await;
    (
        StatusCode::OK,
        Json(common::DocumentsResponse { documents }),
    )
        .into_response()
}

pub(crate) async fn create_document_upload_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
    Json(req): Json<CreateDocumentRequest>,
) -> Response {
    if let Err(error) =
        authorize_workspace_notebook_str(state.auth(), index_permission(), &notebook_id)
    {
        return app_error_response(error);
    }
    match state.create_document_upload(&notebook_id, req).await {
        Ok(resp) => (StatusCode::CREATED, Json(resp)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn add_url_source_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
    Json(req): Json<AddUrlSourceRequest>,
) -> Response {
    if let Err(error) =
        authorize_workspace_notebook_str(state.auth(), index_permission(), &notebook_id)
    {
        return app_error_response(error);
    }
    match state.add_url_source(&notebook_id, req).await {
        Ok(resp) => (StatusCode::CREATED, Json(resp)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn list_sources_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Query(params): Query<ChatSessionsQuery>,
) -> Response {
    if let Err(error) =
        authorize_workspace_query_optional_notebook(state.auth(), params.notebook_id.as_deref())
    {
        return app_error_response(error);
    }
    let sources = state.list_sources(params.notebook_id.as_deref()).await;
    (StatusCode::OK, Json(common::SourcesResponse { sources })).into_response()
}

pub(crate) async fn update_document_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(document_id): Path<String>,
    Json(req): Json<UpdateDocumentRequest>,
) -> Response {
    if let Err(error) = authorize_document_access(&state, &document_id, index_permission()).await {
        return app_error_response(error);
    }
    match state.update_document(&document_id, req).await {
        Ok(_) => {
            let document = state
                .list_documents(None, Some(&document_id))
                .await
                .into_iter()
                .next();
            match document {
                Some(document) => (StatusCode::OK, Json(document)).into_response(),
                None => error_response(
                    StatusCode::NOT_FOUND,
                    "document_not_found",
                    "document not found",
                ),
            }
        }
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn delete_document_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(document_id): Path<String>,
) -> Response {
    if let Err(error) = authorize_document_access(&state, &document_id, index_permission()).await {
        return app_error_response(error);
    }
    match state.delete_document(&document_id).await {
        Ok(_) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn get_document_status_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(document_id): Path<String>,
) -> Response {
    if let Err(error) = authorize_document_access_index_or_query(&state, &document_id).await {
        return app_error_response(error);
    }
    let document = state
        .list_documents(None, Some(&document_id))
        .await
        .into_iter()
        .next();
    match document {
        Some(document) => (
            StatusCode::OK,
            Json(contracts::documents::DocumentStatusResponse {
                status: document.status.as_str().to_string(),
            }),
        )
            .into_response(),
        None => error_response(
            StatusCode::NOT_FOUND,
            "document_not_found",
            "document not found",
        ),
    }
}

pub(crate) async fn get_document_content_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(document_id): Path<String>,
) -> Response {
    if let Err(error) = authorize_document_access_index_or_query(&state, &document_id).await {
        return app_error_response(error);
    }
    match state.get_document_content(&document_id).await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(error) => app_error_response(error),
    }
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct ParsedPreviewQuery {
    pub cursor: Option<usize>,
    pub limit: Option<usize>,
}

pub(crate) async fn get_parsed_preview_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(document_id): Path<String>,
    Query(params): Query<ParsedPreviewQuery>,
) -> Response {
    if let Err(error) = authorize_document_access_index_or_query(&state, &document_id).await {
        return app_error_response(error);
    }
    match state
        .get_parsed_preview(
            &document_id,
            params.cursor.unwrap_or(0),
            params.limit.unwrap_or(50),
        )
        .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn reindex_document_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(document_id): Path<String>,
) -> Response {
    if let Err(error) = authorize_document_access(&state, &document_id, index_permission()).await {
        return app_error_response(error);
    }
    match state.reindex_document(&document_id).await {
        Ok(_) => (
            StatusCode::ACCEPTED,
            Json(contracts::auth::EmptyResponse {}),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn complete_document_upload_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(document_id): Path<String>,
) -> Response {
    if let Err(error) = authorize_document_access(&state, &document_id, index_permission()).await {
        return app_error_response(error);
    }
    match state.complete_document_upload(&document_id).await {
        Ok(_) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}
