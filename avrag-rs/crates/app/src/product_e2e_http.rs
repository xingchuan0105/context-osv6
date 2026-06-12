//! HTTP router for product E2E integration tests (`product-e2e` feature only).

pub fn build_router(state: app_bootstrap::AppState) -> axum::Router {
    transport_http::build_router(state)
}
