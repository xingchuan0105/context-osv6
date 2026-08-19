//! Workspace publish HTTP handlers (ADR-0010 B3b).

use axum::{
    Json,
    body::Bytes,
    extract::{Extension, Path, Query},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::super::app_error_response;
use crate::auth_guard::require_user_session;
use crate::middleware::RequestState;

const EXPORT_ZSTD_LEVEL: i32 = 3;

fn accepts_zstd(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value.split(',').any(|part| {
                part.trim()
                    .split(';')
                    .next()
                    .is_some_and(|token| token.eq_ignore_ascii_case("zstd"))
            })
        })
        .unwrap_or(false)
}

fn json_payload_response<T: Serialize>(headers: &HeaderMap, payload: &T) -> Response {
    let json = match serde_json::to_vec(payload) {
        Ok(bytes) => bytes,
        Err(err) => {
            return app_error_response(common::AppError::internal(format!(
                "serialize publish payload: {err}"
            )));
        }
    };
    if !accepts_zstd(headers) {
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            json,
        )
            .into_response();
    }
    match zstd::encode_all(&json[..], EXPORT_ZSTD_LEVEL) {
        Ok(compressed) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "application/json"),
                (header::CONTENT_ENCODING, "zstd"),
            ],
            compressed,
        )
            .into_response(),
        Err(err) => app_error_response(common::AppError::internal(format!(
            "zstd compress publish export: {err}"
        ))),
    }
}

fn require_session(state: &app_bootstrap::AppState) -> Result<(), Response> {
    require_user_session(
        state.auth(),
        "publish requires a signed-in user session",
    )
    .map_err(app_error_response)
}

fn content_encoding(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::CONTENT_ENCODING)
        .and_then(|value| value.to_str().ok())
}

#[derive(Debug, Deserialize)]
pub(crate) struct PublishStatusQuery {
    pub local_workspace_id: Uuid,
}

pub(crate) async fn create_publish_session_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(body): Json<app_core::CreatePublishSessionRequest>,
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
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(error) = require_session(&state) {
        return error;
    }
    match state
        .workspace()
        .put_publish_part(upload_id, part_n, body.to_vec(), content_encoding(&headers))
        .await
    {
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
    headers: HeaderMap,
) -> Response {
    if let Err(error) = require_session(&state) {
        return error;
    }
    match state.workspace().export_publish_list(workspace_id).await {
        Ok(payload) => json_payload_response(&headers, &payload),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn export_publish_document_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((workspace_id, document_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Response {
    if let Err(error) = require_session(&state) {
        return error;
    }
    match state
        .workspace()
        .export_publish_document(workspace_id, document_id)
        .await
    {
        Ok(payload) => json_payload_response(&headers, &payload),
        Err(error) => app_error_response(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_zstd_from_accept_encoding() {
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT_ENCODING, "gzip, zstd".parse().unwrap());
        assert!(accepts_zstd(&headers));
        headers.insert(header::ACCEPT_ENCODING, "gzip, deflate".parse().unwrap());
        assert!(!accepts_zstd(&headers));
    }

    #[test]
    fn export_payload_is_zstd_only_when_accepted() {
        let payload = serde_json::json!({ "ok": true });
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT_ENCODING, "zstd".parse().unwrap());
        let compressed = json_payload_response(&headers, &payload);
        assert_eq!(
            compressed
                .headers()
                .get(header::CONTENT_ENCODING)
                .map(|value| value.as_bytes()),
            Some(&b"zstd"[..])
        );
        let plain = json_payload_response(&HeaderMap::new(), &payload);
        assert!(plain.headers().get(header::CONTENT_ENCODING).is_none());
    }
}
