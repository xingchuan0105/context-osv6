use app::AppState;
use axum::{
    Router,
    routing::{get, post},
};

use crate::handlers;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/chat", post(handlers::chat_post_handler))
        .route(
            "/chat/sessions",
            get(handlers::list_chat_sessions_handler).post(handlers::create_chat_session_handler),
        )
        .route(
            "/chat/sessions/{session_id}",
            get(handlers::get_chat_session_handler)
                .put(handlers::update_chat_session_handler)
                .delete(handlers::delete_chat_session_handler),
        )
        .route(
            "/chat/sessions/{session_id}/messages",
            get(handlers::get_chat_messages_handler),
        )
        .route(
            "/chat/sessions/{session_id}/messages/{message_id}/feedback",
            post(handlers::message_feedback_handler),
        )
        .route(
            "/chat/citations/lookup",
            post(handlers::citation_lookup_handler),
        )
        .route(
            "/chat/citations/assets/{asset_id}",
            get(handlers::citation_asset_handler),
        )
        .route("/search", get(handlers::search_handler))
        .route("/agent/capabilities", get(handlers::agent_capabilities_handler))
}

pub(crate) fn compat_router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/notebooks/{notebook_id}/chat/completions",
            post(crate::openai_chat_completions_handler),
        )
        .route("/mcp/notebooks/{notebook_id}", get(crate::mcp_sse_handler))
        .route(
            "/mcp/notebooks/{notebook_id}/tools/call",
            post(crate::mcp_tool_call_handler),
        )
}
