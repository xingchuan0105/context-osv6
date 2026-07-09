use axum::Json;
use axum::extract::Extension;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use tracing::warn;

use crate::auth_types::UserPreferencesPayload;
use crate::handlers;
use crate::middleware::RequestState;

pub(crate) async fn auth_get_preferences_handler(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Response {
    if state.auth().actor_id().is_none() {
        return handlers::error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Not authenticated",
        );
    }
    if let Err(error) = crate::auth_guard::forbid_api_key(
        state.auth(),
        "user preferences require a signed-in user session",
    ) {
        return handlers::app_error_response(error);
    }

    match state.current_user_preferences().await {
        Ok(preferences) => {
            let payload = serde_json::to_value(preferences)
                .ok()
                .and_then(|value| serde_json::from_value::<UserPreferencesPayload>(value).ok())
                .unwrap_or_default();
            (StatusCode::OK, Json(payload)).into_response()
        }
        Err(error) => {
            warn!(error = %error, "failed to load user preferences");
            handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to load preferences",
            )
        }
    }
}

pub(crate) async fn auth_update_preferences_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(req): Json<UserPreferencesPayload>,
) -> Response {
    if state.auth().actor_id().is_none() {
        return handlers::error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Not authenticated",
        );
    }
    if let Err(error) = crate::auth_guard::forbid_api_key(
        state.auth(),
        "user preferences require a signed-in user session",
    ) {
        return handlers::app_error_response(error);
    }

    let previous_preferences = match state.current_user_preferences().await {
        Ok(preferences) => serde_json::to_value(preferences)
            .ok()
            .and_then(|value| serde_json::from_value::<UserPreferencesPayload>(value).ok())
            .unwrap_or_default(),
        Err(error) => {
            warn!(error = %error, "failed to load existing preferences");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to save preferences",
            );
        }
    };

    let next_preferences = match serde_json::to_value(&req)
        .ok()
        .and_then(|value| serde_json::from_value::<contracts::preferences::UserPreferences>(value).ok())
    {
        Some(preferences) => preferences,
        None => {
            return handlers::error_response(
                StatusCode::BAD_REQUEST,
                "validation_error",
                "Invalid preferences payload",
            );
        }
    };

    if let Err(error) = state.save_current_user_preferences(&next_preferences).await {
        warn!(error = %error, "failed to persist user preferences");
        return handlers::error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Failed to save preferences",
        );
    }

    for (notebook_id, notes) in changed_workspace_drafts(&previous_preferences, &req) {
        let metadata = serde_json::json!({
            "notes_length": notes.chars().count(),
            "synced": true,
        });
        state
            .record_product_event_if_available(
                analytics::ProductEventName::NoteEdited,
                analytics::Surface::Workspace,
                analytics::ResultTag::Success,
                None,
                uuid::Uuid::parse_str(&notebook_id).ok(),
                metadata.clone(),
            )
            .await;
        state
            .record_product_event_if_available(
                analytics::ProductEventName::NoteSynced,
                analytics::Surface::Workspace,
                analytics::ResultTag::Success,
                None,
                uuid::Uuid::parse_str(&notebook_id).ok(),
                metadata,
            )
            .await;
    }

    (StatusCode::OK, Json(req)).into_response()
}

pub(crate) async fn auth_get_agent_preferences_handler(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Response {
    if state.auth().actor_id().is_none() {
        return handlers::error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Not authenticated",
        );
    }
    if let Err(error) = crate::auth_guard::forbid_api_key(
        state.auth(),
        "user preferences require a signed-in user session",
    ) {
        return handlers::app_error_response(error);
    }

    match state.current_user_preferences().await {
        Ok(preferences) => (StatusCode::OK, Json(preferences.agent_memory)).into_response(),
        Err(error) => {
            warn!(error = %error, "failed to load agent preferences");
            handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to load agent preferences",
            )
        }
    }
}

pub(crate) async fn auth_update_agent_preferences_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(agent_memory): Json<contracts::preferences::AgentPreferenceMemory>,
) -> Response {
    if state.auth().actor_id().is_none() {
        return handlers::error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Not authenticated",
        );
    }
    if let Err(error) = crate::auth_guard::forbid_api_key(
        state.auth(),
        "user preferences require a signed-in user session",
    ) {
        return handlers::app_error_response(error);
    }

    let mut preferences = match state.current_user_preferences().await {
        Ok(preferences) => preferences,
        Err(error) => {
            warn!(error = %error, "failed to load agent preferences before update");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to save agent preferences",
            );
        }
    };
    preferences.agent_memory = agent_memory;

    match state.save_current_user_preferences(&preferences).await {
        Ok(saved) => (StatusCode::OK, Json(saved.agent_memory)).into_response(),
        Err(error) => {
            warn!(error = %error, "failed to save agent preferences");
            handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to save agent preferences",
            )
        }
    }
}

pub(crate) async fn auth_delete_agent_preference_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(preference_id): Path<String>,
) -> Response {
    if state.auth().actor_id().is_none() {
        return handlers::error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Not authenticated",
        );
    }
    if let Err(error) = crate::auth_guard::forbid_api_key(
        state.auth(),
        "user preferences require a signed-in user session",
    ) {
        return handlers::app_error_response(error);
    }

    match state.delete_current_agent_preference(&preference_id).await {
        Ok(Some(agent_memory)) => (StatusCode::OK, Json(agent_memory)).into_response(),
        Ok(None) => handlers::error_response(
            StatusCode::NOT_FOUND,
            "preference_not_found",
            "Agent preference not found",
        ),
        Err(error) => {
            warn!(error = %error, "failed to delete agent preference");
            handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to delete agent preference",
            )
        }
    }
}

fn changed_workspace_drafts(
    previous: &UserPreferencesPayload,
    next: &UserPreferencesPayload,
) -> Vec<(String, String)> {
    let previous_map = previous
        .dashboard
        .workspace_drafts
        .iter()
        .map(|draft| (draft.notebook_id.clone(), draft.notes.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let next_map = next
        .dashboard
        .workspace_drafts
        .iter()
        .map(|draft| (draft.notebook_id.clone(), draft.notes.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();

    let mut notebook_ids = previous_map.keys().cloned().collect::<std::collections::BTreeSet<_>>();
    notebook_ids.extend(next_map.keys().cloned());

    notebook_ids
        .into_iter()
        .filter_map(|notebook_id| {
            let before = previous_map.get(&notebook_id).cloned().unwrap_or_default();
            let after = next_map.get(&notebook_id).cloned().unwrap_or_default();
            (before != after).then_some((notebook_id, after))
        })
        .collect()
}
