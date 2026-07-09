//! User notification HTTP handlers.

use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use super::super::app_error_response;
use crate::auth_guard::require_user_session;
use crate::middleware::RequestState;

pub(crate) async fn list_notifications_handler(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Response {
    if let Err(error) = require_user_session(
        state.auth(),
        "this endpoint requires a signed-in user session",
    ) {
        return app_error_response(error);
    }
    match state.list_notifications(100, 0).await {
        Ok(notifications) => (
            StatusCode::OK,
            Json(common::NotificationsResponse { notifications }),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn mark_notification_read_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notification_id): Path<String>,
) -> Response {
    if let Err(error) = require_user_session(
        state.auth(),
        "this endpoint requires a signed-in user session",
    ) {
        return app_error_response(error);
    }
    match state.mark_notification_read(&notification_id).await {
        Ok(_) => (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response(),
        Err(error) => app_error_response(error),
    }
}
