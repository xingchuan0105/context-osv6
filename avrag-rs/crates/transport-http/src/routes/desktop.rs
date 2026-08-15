//! Desktop relay token routes (W2: cloud login → mint relay credentials).
//!
//! Session-JWT only (the normal cloud login token): workspace API keys are
//! rejected. Tokens authorize ONLY `/v1/relay/*` — never the rest of the API.

use app_bootstrap::AppState;
use axum::{
    Extension, Json, Router,
    extract::Path,
    routing::post,
};
use common::ApiResponse;
use uuid::Uuid;

use crate::middleware::RequestState;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/desktop/tokens",
            post(mint_desktop_token).get(list_desktop_tokens),
        )
        .route("/desktop/tokens/{id}/revoke", post(revoke_desktop_token))
}

async fn mint_desktop_token(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(body): Json<app_core::MintDesktopTokenRequest>,
) -> Json<ApiResponse<app_core::MintedDesktopTokenResponse>> {
    if let Err(error) = crate::auth_guard::forbid_api_key(
        state.auth(),
        "desktop tokens require a signed-in user session, not a workspace API key",
    ) {
        return Json(ApiResponse::err(error.code(), error.message()));
    }
    Json(state.desktop_api().mint_token(&body.name).await)
}

async fn list_desktop_tokens(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Json<ApiResponse<Vec<app_core::DesktopTokenView>>> {
    if let Err(error) = crate::auth_guard::forbid_api_key(
        state.auth(),
        "desktop tokens require a signed-in user session, not a workspace API key",
    ) {
        return Json(ApiResponse::err(error.code(), error.message()));
    }
    Json(state.desktop_api().list_tokens().await)
}

async fn revoke_desktop_token(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(id): Path<Uuid>,
) -> Json<ApiResponse<app_core::DesktopTokenView>> {
    if let Err(error) = crate::auth_guard::forbid_api_key(
        state.auth(),
        "desktop tokens require a signed-in user session, not a workspace API key",
    ) {
        return Json(ApiResponse::err(error.code(), error.message()));
    }
    Json(state.desktop_api().revoke_token(id).await)
}
