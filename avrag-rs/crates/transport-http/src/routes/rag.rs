use app::AppState;
use axum::{Router, routing::post};

use crate::handlers;

pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/rag/execute-plan", post(handlers::rag_execute_plan_handler))
}
