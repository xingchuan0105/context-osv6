use app_bootstrap::AppState;
use axum::{Router, routing::post};

use crate::handlers;

pub(crate) fn router() -> Router<AppState> {
    // ADR 0006: `/rag/execute-plan` product surface removed (was 410 Gone).
    // Clients that still call it receive framework 404.
    Router::new().route("/runtime/execute", post(handlers::runtime_execute_handler))
}
