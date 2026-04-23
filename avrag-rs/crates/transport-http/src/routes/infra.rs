use app::AppState;
use axum::{
    Router,
    routing::{get, post, put},
};

use crate::handlers;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(crate::health_handler))
        .route("/ready", get(crate::ready_handler))
        .route("/metrics", get(crate::metrics_handler))
        .route("/docs", get(crate::docs_handler))
        .route("/openapi.json", get(crate::openapi_handler))
        .route("/dev-upload/{document_id}", put(crate::dev_upload_handler))
        .route("/uploads/{document_id}", put(crate::signed_upload_handler))
        .route("/webhooks/stripe", post(crate::billing_webhook_handler))
        .route(
            "/api/v1/share/validate/{token}",
            get(handlers::validate_share_token_handler),
        )
        .route(
            "/api/shared/kb/{token}",
            get(crate::shared_notebook_handler),
        )
}
