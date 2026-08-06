//! MCP share tools — user session only; reuses ShareApp / ADR-0010 quotas.

use app_bootstrap::AppState;
use common::AppError;
use serde_json::{Value, json};

use crate::auth_guard::{
    ensure_user_workspace_access, require_user_session, require_workspace_id_arg,
};
use crate::mcp::catalog;

fn require_share_user(state: &AppState) -> Result<(), AppError> {
    require_user_session(
        state.auth(),
        "share tools require a signed-in user session (CONTEXT_OS_USER_TOKEN), not a workspace API key",
    )
}

async fn require_share_workspace(state: &AppState, workspace_id: &str) -> Result<(), AppError> {
    require_share_user(state)?;
    if !state.postgres_configured() {
        return Err(AppError::internal("postgres backend is not configured"));
    }
    ensure_user_workspace_access(state, workspace_id).await
}

/// Create a share link (enables share_enabled subject to plan quota).
pub(crate) async fn share_create_link(
    state: &AppState,
    arguments: &Value,
) -> Result<Value, AppError> {
    let workspace_uuid = require_workspace_id_arg(arguments)?;
    let workspace_id = workspace_uuid.to_string();
    require_share_workspace(state, &workspace_id).await?;

    let role = arguments
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or("viewer")
        .trim();
    let access_level = avrag_share::AccessLevel::from_role(role);

    let expires_in_secs = arguments
        .get("expires_in_secs")
        .and_then(|v| v.as_i64())
        .filter(|n| *n > 0)
        .or_else(|| {
            arguments
                .get("expires_at")
                .and_then(|v| v.as_str())
                .and_then(|raw| {
                    let expires_at = chrono::DateTime::parse_from_rfc3339(raw).ok()?;
                    let delta = expires_at
                        .with_timezone(&chrono::Utc)
                        .signed_duration_since(chrono::Utc::now())
                        .num_seconds();
                    (delta > 0).then_some(delta)
                })
        });

    let result = state
        .share()
        .create_share_link(workspace_id.clone(), access_level, expires_in_secs)
        .await?;

    Ok(catalog::success_result(
        "workspace.share_create_link",
        Some(&workspace_id),
        json!(result),
        vec![
            "workspace.share_get_settings to inspect tokens and access_level",
            "account.share_quota to see plan share-slot usage",
        ],
    ))
}

pub(crate) async fn share_get_settings(
    state: &AppState,
    arguments: &Value,
) -> Result<Value, AppError> {
    let workspace_uuid = require_workspace_id_arg(arguments)?;
    let workspace_id = workspace_uuid.to_string();
    require_share_workspace(state, &workspace_id).await?;

    let settings = state.share().get_share_settings(workspace_id.clone()).await?;
    Ok(catalog::success_result(
        "workspace.share_get_settings",
        Some(&workspace_id),
        serde_json::to_value(settings).unwrap_or_else(|_| json!({})),
        vec!["workspace.share_update_settings to change access_level or question limits"],
    ))
}

pub(crate) async fn share_update_settings(
    state: &AppState,
    arguments: &Value,
) -> Result<Value, AppError> {
    let workspace_uuid = require_workspace_id_arg(arguments)?;
    let workspace_id = workspace_uuid.to_string();
    require_share_workspace(state, &workspace_id).await?;

    let access_level = arguments
        .get("access_level")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let allow_download = arguments.get("allow_download").and_then(|v| v.as_bool());
    let anon_question_limit = arguments
        .get("anon_question_limit")
        .and_then(|v| v.as_i64())
        .map(|n| n as i32);
    // member_question_limit: omit = no change; null = clear to unlimited; number = set.
    let member_limit: Option<Option<i32>> = if arguments
        .as_object()
        .is_some_and(|o| o.contains_key("member_question_limit"))
    {
        match arguments.get("member_question_limit") {
            Some(v) if v.is_null() => Some(None),
            Some(v) => {
                let Some(n) = v.as_i64() else {
                    return Err(AppError::validation(
                        "invalid_member_question_limit",
                        "member_question_limit must be an integer or null",
                    ));
                };
                Some(Some(n as i32))
            }
            None => None,
        }
    } else {
        None
    };

    let settings = state
        .share()
        .update_share_settings(
            workspace_id.clone(),
            access_level,
            allow_download,
            anon_question_limit,
            member_limit,
        )
        .await?;

    Ok(catalog::success_result(
        "workspace.share_update_settings",
        Some(&workspace_id),
        serde_json::to_value(settings).unwrap_or_else(|_| json!({})),
        vec!["account.share_quota when access_level enables sharing (link/public)"],
    ))
}

pub(crate) async fn share_revoke_link(
    state: &AppState,
    arguments: &Value,
) -> Result<Value, AppError> {
    let workspace_uuid = require_workspace_id_arg(arguments)?;
    let workspace_id = workspace_uuid.to_string();
    require_share_workspace(state, &workspace_id).await?;

    let token = arguments
        .get("token")
        .or_else(|| arguments.get("share_token"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if token.is_empty() {
        return Err(AppError::validation(
            "token_required",
            "workspace.share_revoke_link requires arguments.token",
        ));
    }

    state.share().revoke_share_link(token.clone()).await?;
    Ok(catalog::success_result(
        "workspace.share_revoke_link",
        Some(&workspace_id),
        json!({ "revoked": true, "token": token }),
        vec!["workspace.share_get_settings to confirm"],
    ))
}

pub(crate) async fn share_quota(state: &AppState, _arguments: &Value) -> Result<Value, AppError> {
    require_share_user(state)?;
    if !state.postgres_configured() {
        return Err(AppError::internal("postgres backend is not configured"));
    }
    let quota = state.share().get_share_quota().await?;
    Ok(catalog::success_result(
        "account.share_quota",
        None,
        serde_json::to_value(quota).unwrap_or_else(|_| json!({})),
        vec![
            "workspace.share_create_link occupies a share-enabled slot on the owner plan",
            "upgrade plan when used >= max (SHARE_WORKSPACE_QUOTA_EXCEEDED)",
        ],
    ))
}

pub(crate) async fn share_invite_member(
    state: &AppState,
    arguments: &Value,
) -> Result<Value, AppError> {
    let workspace_uuid = require_workspace_id_arg(arguments)?;
    let workspace_id = workspace_uuid.to_string();
    require_share_workspace(state, &workspace_id).await?;

    let email = arguments
        .get("email")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if email.is_empty() {
        return Err(AppError::validation(
            "email_required",
            "workspace.share_invite_member requires arguments.email",
        ));
    }
    let role = arguments
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or("viewer")
        .trim();
    let access_level = avrag_share::AccessLevel::from_role(role);

    let member = state
        .share()
        .invite_share_member(workspace_id.clone(), email.clone(), access_level)
        .await?;

    Ok(catalog::success_result(
        "workspace.share_invite_member",
        Some(&workspace_id),
        json!({
            "member_id": member.id,
            "email": email,
            "role": role,
            "invite_status": member.invite_status,
        }),
        vec!["workspace.share_get_settings to list members"],
    ))
}
