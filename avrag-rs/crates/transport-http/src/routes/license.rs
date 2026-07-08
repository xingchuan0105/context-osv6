use app_bootstrap::AppState;
use axum::{
    Extension, Json, Router,
    extract::Path,
    routing::{delete, get, post},
};
use common::{ApiResponse, UserId};
use std::sync::{Arc, LazyLock};

use crate::middleware::RequestState;

static LICENSE_CLIENT: LazyLock<Option<Arc<avrag_licensing::KeygenClient>>> = LazyLock::new(|| {
    match avrag_licensing::KeygenClient::from_env() {
        Ok(client) if client.config().enabled() => Some(Arc::new(client)),
        _ => None,
    }
});

fn require_client() -> Result<Arc<avrag_licensing::KeygenClient>, ApiResponse<serde_json::Value>> {
    LICENSE_CLIENT.clone().ok_or_else(|| {
        ApiResponse::err(
            "licensing_not_configured",
            "Keygen licensing is not configured",
        )
    })
}

fn require_user(state: &AppState) -> Result<UserId, ApiResponse<serde_json::Value>> {
    let Some(actor_id) = state.auth().actor_id() else {
        return Err(ApiResponse::err(
            "authenticated_user_required",
            "authenticated user required",
        ));
    };
    Ok(UserId::from(actor_id.into_uuid()))
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/licenses/checkout", post(create_checkout))
        .route("/licenses/me", get(list_my_licenses))
        .route("/licenses/trial", post(create_trial))
        .route("/licenses/{id}/machines", get(list_machines))
        .route("/licenses/{id}/machines/{mid}", delete(deactivate_machine))
}

async fn create_checkout(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(body): Json<avrag_licensing::CreateLicenseCheckoutRequest>,
) -> Json<ApiResponse<avrag_licensing::CreateLicenseCheckoutResponse>> {
    let Ok(client) = require_client() else {
        return Json(ApiResponse::err(
            "licensing_not_configured",
            "Keygen licensing is not configured",
        ));
    };
    let Ok(user_id) = require_user(&state) else {
        return Json(ApiResponse::err(
            "authenticated_user_required",
            "authenticated user required",
        ));
    };
    Json(avrag_licensing::handle_create_license_checkout(&client, user_id, body).await)
}

async fn list_my_licenses(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Json<ApiResponse<avrag_licensing::LicenseListResponse>> {
    let Ok(client) = require_client() else {
        return Json(ApiResponse::err(
            "licensing_not_configured",
            "Keygen licensing is not configured",
        ));
    };
    let Ok(user_id) = require_user(&state) else {
        return Json(ApiResponse::err(
            "authenticated_user_required",
            "authenticated user required",
        ));
    };
    Json(avrag_licensing::handle_list_user_licenses(&client, user_id).await)
}

async fn create_trial(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Json<ApiResponse<avrag_licensing::LicenseCheckoutResponse>> {
    let Ok(client) = require_client() else {
        return Json(ApiResponse::err(
            "licensing_not_configured",
            "Keygen licensing is not configured",
        ));
    };
    let Ok(user_id) = require_user(&state) else {
        return Json(ApiResponse::err(
            "authenticated_user_required",
            "authenticated user required",
        ));
    };
    Json(avrag_licensing::handle_create_trial_license(&client, user_id).await)
}

async fn list_machines(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<avrag_licensing::LicenseMachineListResponse>> {
    let Ok(client) = require_client() else {
        return Json(ApiResponse::err(
            "licensing_not_configured",
            "Keygen licensing is not configured",
        ));
    };
    let Ok(_user_id) = require_user(&state) else {
        return Json(ApiResponse::err(
            "authenticated_user_required",
            "authenticated user required",
        ));
    };
    Json(avrag_licensing::handle_list_license_machines(&client, &id).await)
}

async fn deactivate_machine(
    Extension(RequestState(state)): Extension<RequestState>,
    Path((id, mid)): Path<(String, String)>,
) -> Json<ApiResponse<serde_json::Value>> {
    let Ok(client) = require_client() else {
        return Json(ApiResponse::err(
            "licensing_not_configured",
            "Keygen licensing is not configured",
        ));
    };
    let Ok(_user_id) = require_user(&state) else {
        return Json(ApiResponse::err(
            "authenticated_user_required",
            "authenticated user required",
        ));
    };
    let _ = id;
    Json(avrag_licensing::handle_deactivate_machine(&client, &mid).await)
}
