//! Best-effort in-app notification helpers (ADR-0010 W4).

use app_bootstrap::AppState;
use app_core::NotificationCreateParams;
use contracts::auth_runtime::AuthContext;
use uuid::Uuid;

/// Emit a user notification when chat persistence is available. Never fails the request.
pub async fn emit_user_notification(
    state: &AppState,
    auth: &AuthContext,
    user_id: Uuid,
    event_type: &str,
    title: &str,
    body: &str,
    data: serde_json::Value,
) {
    let Some(pg) = state.storage().chat_persistence() else {
        return;
    };
    if let Err(e) = pg
        .create_notification(
            auth,
            NotificationCreateParams {
                user_id,
                event_type: event_type.to_string(),
                title: title.to_string(),
                body: body.to_string(),
                data,
                channels: vec!["in_app".to_string()],
            },
        )
        .await
    {
        tracing::warn!(error = %e, %event_type, %user_id, "emit_user_notification failed");
    }
}
