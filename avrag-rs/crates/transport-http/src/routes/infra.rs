use app_bootstrap::AppState;
use axum::{
    Router,
    routing::{get, post, put},
};

use crate::handlers;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(crate::lib_impl::health_handler))
        .route("/ready", get(crate::lib_impl::ready_handler))
        .route("/metrics", get(crate::lib_impl::metrics_handler))
        .route("/docs", get(crate::lib_impl::docs_handler))
        .route("/openapi.json", get(crate::lib_impl::openapi_handler))
        // `/dev-upload` is registered in `router_core` with auth middleware.
        // Upload byte path: keeps the large body allowance when the global
        // DefaultBodyLimit is tightened (router_core).
        .route(
            "/uploads/{document_id}",
            put(crate::lib_impl::signed_upload_handler)
                .layer(tower_http::limit::RequestBodyLimitLayer::new(512 * 1024 * 1024)),
        )
        .route("/webhooks/{provider}", post(crate::lib_impl::billing_webhook_handler))
        .route(
            "/webhooks/object-storage",
            post(crate::lib_impl::object_storage_webhook_handler),
        )
        .route(
            "/api/v1/share/validate/{token}",
            get(handlers::validate_share_token_handler),
        )
        .route(
            "/api/shared/kb/{token}",
            get(crate::lib_impl::shared_workspace_handler),
        )
        .route(
            "/api/public/users/{user_id}/media/{kind}",
            get(crate::lib_impl::public_user_media_handler),
        )
        .route(
            "/api/public/users/{user_id}/shares",
            get(crate::lib_impl::public_user_shares_handler),
        )
    }
