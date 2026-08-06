//! ADR-0010 §1.2 / B6: desktop license + Keygen product paths retired.
//! Routes stay as explicit **gone** stubs so old clients receive a product message.

use app_bootstrap::AppState;
use axum::{Json, Router, routing::delete, routing::get, routing::post};
use common::ApiResponse;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/licenses/checkout", post(license_product_retired))
        .route("/licenses/me", get(license_product_retired))
        .route("/licenses/trial", post(license_product_retired))
        .route("/licenses/{id}/machines", get(license_product_retired))
        .route(
            "/licenses/{id}/machines/{mid}",
            delete(license_product_retired),
        )
}

async fn license_product_retired() -> Json<ApiResponse<serde_json::Value>> {
    Json(ApiResponse::err(
        "license_product_retired",
        "Desktop license sales and Keygen activation are retired (ADR-0010). The client is free; use cloud wallet/BYOK for model spend.",
    ))
}
