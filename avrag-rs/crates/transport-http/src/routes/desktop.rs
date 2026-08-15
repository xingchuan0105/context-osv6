//! Desktop relay token routes (W2: cloud login → mint relay credentials).
//!
//! Session-JWT only (the normal cloud login token): workspace API keys are
//! rejected. Tokens authorize ONLY `/v1/relay/*` — never the rest of the API.

use app_bootstrap::AppState;
use axum::{
    Extension, Json, Router,
    extract::Path,
    routing::{get, post},
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
        .route("/desktop/relay-config", get(desktop_relay_config))
}

/// Relay coordinates for the desktop shell (W3): server-driven so the client
/// never hardcodes platform models. `relay_base_url` derives from the public
/// base config; models are the platform pinned pools (same source the relay
/// itself uses, `AppConfig::from_env`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct DesktopRelayConfigView {
    pub relay_base_url: String,
    pub chat_model: String,
    pub embedding_model: String,
}

async fn desktop_relay_config(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Json<ApiResponse<DesktopRelayConfigView>> {
    if let Err(error) = crate::auth_guard::forbid_api_key(
        state.auth(),
        "desktop relay config requires a signed-in user session, not a workspace API key",
    ) {
        return Json(ApiResponse::err(error.code(), error.message()));
    }
    let config = app_core::AppConfig::from_env();
    let base = config.public_base_url.trim().trim_end_matches('/');
    Json(ApiResponse::ok(DesktopRelayConfigView {
        relay_base_url: format!("{base}/v1/relay"),
        chat_model: config.agent_llm.model,
        embedding_model: config.embedding.model,
    }))
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
