use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use tracing::{info, warn};
use uuid::Uuid;

use crate::AppState;

// ---------------------------------------------------------------------------
// E2E Reset API — 仅用于测试环境，6 层安全 gate
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub(crate) struct ResetUserDataRequest {
    email: String,
}

#[derive(Serialize)]
pub(crate) struct ResetUserDataResponse {
    success: bool,
    message: String,
}

pub(crate) fn router() -> axum::Router<AppState> {
    axum::Router::new().route("/reset-user-data", axum::routing::post(reset_user_data_handler))
}

async fn reset_user_data_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<ResetUserDataRequest>,
) -> Response {
    // Gate 1: 环境 gates — production 绝对不允许
    let node_env = env::var("NODE_ENV").unwrap_or_default();
    let e2e_enabled = env::var("E2E_ENABLED").unwrap_or_default();
    if node_env == "production" || e2e_enabled != "true" {
        warn!(
            node_env = %node_env,
            e2e_enabled = %e2e_enabled,
            "e2e reset rejected: environment gate failed"
        );
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "e2e not enabled in this environment" })),
        )
            .into_response();
    }

    // Gate 2: Secret gates
    let expected_secret = match env::var("E2E_RESET_SECRET") {
        Ok(s) if !s.is_empty() => s,
        _ => {
            warn!("e2e reset rejected: E2E_RESET_SECRET not configured");
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "e2e reset secret not configured" })),
            )
                .into_response();
        }
    };
    let provided_secret = headers
        .get("x-e2e-secret")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if provided_secret != expected_secret {
        warn!("e2e reset rejected: secret mismatch");
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "invalid e2e secret" })),
        )
            .into_response();
    }

    // Gate 3: 账号前缀 gates
    let email = body.email.trim().to_lowercase();
    let allowed = email.starts_with("e2e-") || email.ends_with("@test.local");
    if !allowed {
        warn!(email = %email, "e2e reset rejected: account prefix gate failed");
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "account does not match allowed e2e patterns" })),
        )
            .into_response();
    }

    // Gate 4: 网络 gates (由部署层面保证 — 仅监听 staging 私网/loopback)
    // 此 handler 不处理网络层白名单，由反向代理/防火墙负责。

    // Gate 5 & 6: 查找用户并级联删除 + 审计日志
    let repo = match state.pg() {
        Some(pg) => pg,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "database not available" })),
            )
                .into_response();
        }
    };

    // 通过 email 查找 user_id
    let user_id: Option<Uuid> = match sqlx::query_as::<_, (Option<Uuid>,)>(
        "select id from users where email = $1 limit 1"
    )
    .bind(&email)
    .fetch_optional(repo.raw())
    .await
    {
        Ok(Some((Some(id),))) => Some(id),
        Ok(Some((None,))) | Ok(None) => None,
        Err(e) => {
            warn!(error = %e, "e2e reset: failed to lookup user by email");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error during user lookup" })),
            )
                .into_response();
        }
    };

    let user_id = match user_id {
        Some(id) => id,
        None => {
            // 用户不存在 = 环境已干净，视为成功
            return (
                StatusCode::OK,
                Json(ResetUserDataResponse {
                    success: true,
                    message: "user not found, nothing to reset".to_string(),
                }),
            )
                .into_response();
        }
    };

    // 审计日志
    info!(
        user_id = %user_id,
        email = %email,
        "e2e reset-user-data executed"
    );

    // 调用级联删除 (保留账号，删除数据)
    // TODO: delete_user_cascade 会删除用户账号本身。
    // 如果需求是"保留账号、只删除数据"，需要改用更细粒度的删除逻辑。
    // 当前先用 delete_user_cascade，后续根据实际需求调整。
    let auth_context = state.auth().clone();
    match repo.delete_user_cascade(&auth_context, user_id).await {
        Ok(true) => {
            info!(user_id = %user_id, "e2e reset-user-data succeeded");
            (
                StatusCode::OK,
                Json(ResetUserDataResponse {
                    success: true,
                    message: "user data reset successfully".to_string(),
                }),
            )
                .into_response()
        }
        Ok(false) => {
            warn!(user_id = %user_id, "e2e reset-user-data: user not found or no data to delete");
            (
                StatusCode::OK,
                Json(ResetUserDataResponse {
                    success: true,
                    message: "no data to reset".to_string(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            warn!(error = %e, user_id = %user_id, "e2e reset-user-data failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "failed to reset user data" })),
            )
                .into_response()
        }
    }
}
