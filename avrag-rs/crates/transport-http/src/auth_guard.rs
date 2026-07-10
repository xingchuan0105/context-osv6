use app_bootstrap::AppState;
use contracts::auth_runtime::{AuthContext, AuthError, SubjectKind};
use common::AppError;
use contracts::agent_permissions::{
    PERM_ADMIN, PERM_INDEX, PERM_QUERY, PERM_WORKSPACE_CREATE, PERM_WORKSPACE_LIST,
};
use uuid::Uuid;

pub(crate) fn auth_error_to_app_error(error: AuthError) -> AppError {
    match error {
        AuthError::MissingPermission { permission } => AppError::forbidden(
            "missing_permission",
            format!("missing permission: {permission}"),
        ),
        AuthError::WorkspaceScopeMismatch { expected, actual } => AppError::forbidden(
            "workspace_scope_mismatch",
            format!("API key is scoped to workspace {expected}, got {actual}"),
        ),
        AuthError::MissingWorkspaceScope => {
            AppError::forbidden("missing_workspace_scope", "workspace API key scope required")
        }
        AuthError::CrossTenantAccess => AppError::forbidden(
            "cross_tenant_access",
            "resource belongs to another account",
        ),
        AuthError::MissingUserScope => AppError::unauthorized("account scope required"),
    }
}

fn ensure_permission_for_subject(auth: &AuthContext, permission: &str) -> Result<(), AppError> {
    if matches!(auth.subject_kind(), SubjectKind::User) {
        return Ok(());
    }
    auth.ensure_permission(permission)
        .map_err(auth_error_to_app_error)
}

pub(crate) fn authorize_account_tool(auth: &AuthContext, permission: &str) -> Result<(), AppError> {
    if auth.workspace_id().is_some() {
        return Err(AppError::forbidden(
            "workspace_key_cannot_call_account_tools",
            "workspace-scoped API keys cannot call org-level endpoints",
        ));
    }
    ensure_permission_for_subject(auth, permission)
}

pub(crate) fn authorize_workspace_tool(
    auth: &AuthContext,
    permission: &str,
    workspace_id: Uuid,
) -> Result<(), AppError> {
    if matches!(auth.subject_kind(), SubjectKind::ApiKey) {
        if auth.workspace_id().is_none() {
            return Err(AppError::forbidden(
                "account_key_cannot_call_workspace_tools",
                "org-scoped API keys cannot call workspace-level endpoints",
            ));
        }
        auth.ensure_workspace_scope(workspace_id)
            .map_err(auth_error_to_app_error)?;
    }
    ensure_permission_for_subject(auth, permission)
}

pub(crate) fn authorize_api_key_query_scoped(auth: &AuthContext) -> Result<(), AppError> {
    if !matches!(auth.subject_kind(), SubjectKind::ApiKey) {
        return Ok(());
    }
    ensure_permission_for_subject(auth, PERM_QUERY)?;
    if auth.workspace_id().is_none() {
        return Err(AppError::forbidden(
            "account_key_cannot_call_workspace_tools",
            "org-scoped API keys cannot call workspace-level endpoints",
        ));
    }
    Ok(())
}

pub(crate) fn authorize_workspace_index_or_query(
    auth: &AuthContext,
    workspace_id: Uuid,
) -> Result<(), AppError> {
    if matches!(auth.subject_kind(), SubjectKind::ApiKey) {
        if auth.workspace_id().is_none() {
            return Err(AppError::forbidden(
                "account_key_cannot_call_workspace_tools",
                "org-scoped API keys cannot call workspace-level endpoints",
            ));
        }
        auth.ensure_workspace_scope(workspace_id)
            .map_err(auth_error_to_app_error)?;
        let has_index = auth.ensure_permission(PERM_INDEX).is_ok();
        let has_query = auth.ensure_permission(PERM_QUERY).is_ok();
        if !has_index && !has_query {
            return Err(AppError::forbidden(
                "missing_permission",
                "missing permission: index or query required",
            ));
        }
        return Ok(());
    }
    Ok(())
}

pub(crate) fn authorize_workspace_index_or_query_str(
    auth: &AuthContext,
    workspace_id: &str,
) -> Result<(), AppError> {
    if !matches!(auth.subject_kind(), SubjectKind::ApiKey) {
        return Ok(());
    }
    let notebook_uuid = parse_workspace_id(workspace_id)?;
    authorize_workspace_index_or_query(auth, notebook_uuid)
}

pub(crate) fn forbid_workspace_api_key(auth: &AuthContext, message: &str) -> Result<(), AppError> {
    forbid_api_key(auth, message)
}

pub(crate) fn forbid_api_key(auth: &AuthContext, message: &str) -> Result<(), AppError> {
    if matches!(auth.subject_kind(), SubjectKind::ApiKey) {
        return Err(AppError::forbidden("api_key_forbidden", message));
    }
    Ok(())
}

pub(crate) fn require_user_session(auth: &AuthContext, message: &str) -> Result<(), AppError> {
    forbid_api_key(auth, message)
}

pub(crate) fn require_user_admin(auth: &AuthContext) -> Result<(), AppError> {
    if !matches!(auth.subject_kind(), SubjectKind::User) {
        return Err(AppError::forbidden(
            "admin_required",
            "account admin permission required",
        ));
    }
    auth.ensure_permission(PERM_ADMIN).map_err(|_| {
        AppError::forbidden("admin_required", "account admin permission required")
    })
}

pub(crate) fn authorize_session_notebook(
    auth: &AuthContext,
    session_workspace_id: &str,
) -> Result<(), AppError> {
    if !matches!(auth.subject_kind(), SubjectKind::ApiKey) {
        return Ok(());
    }
    authorize_workspace_notebook_str(auth, query_permission(), session_workspace_id)
}

pub(crate) async fn authorize_session_access(
    state: &AppState,
    session_id: &str,
) -> Result<contracts::workspaces::ChatSession, AppError> {
    let session = state
        .agent()
        .get_session(session_id)
        .await
        .ok_or_else(|| AppError::not_found("session_not_found", "session not found"))?;
    authorize_session_notebook(state.auth(), &session.workspace_id)?;
    Ok(session)
}

pub(crate) async fn ensure_document_in_workspace(
    state: &AppState,
    document_id: &str,
    workspace_id: &str,
) -> Result<(), AppError> {
    let document = state.workspace()
        .list_documents(None, Some(document_id))
        .await
        .into_iter()
        .next()
        .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
    if document.workspace_id != workspace_id {
        return Err(AppError::forbidden(
            "document_workspace_mismatch",
            "document does not belong to the requested workspace",
        ));
    }
    Ok(())
}

pub(crate) fn authorize_workspace_notebook_str(
    auth: &AuthContext,
    permission: &str,
    workspace_id: &str,
) -> Result<(), AppError> {
    if !matches!(auth.subject_kind(), SubjectKind::ApiKey) {
        return Ok(());
    }
    let notebook_uuid = parse_workspace_id(workspace_id)?;
    authorize_workspace_tool(auth, permission, notebook_uuid)
}

pub(crate) fn authorize_workspace_query_optional_notebook(
    auth: &AuthContext,
    workspace_id: Option<&str>,
) -> Result<(), AppError> {
    if !matches!(auth.subject_kind(), SubjectKind::ApiKey) {
        return Ok(());
    }
    let workspace_id = workspace_id
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            AppError::validation(
                "workspace_id_required",
                "workspace_id is required for workspace API keys",
            )
        })?;
    authorize_workspace_notebook_str(auth, PERM_QUERY, workspace_id)
}

pub(crate) async fn authorize_document_access(
    state: &AppState,
    document_id: &str,
    permission: &str,
) -> Result<(), AppError> {
    if !matches!(state.auth().subject_kind(), SubjectKind::ApiKey) {
        return Ok(());
    }
    let document = state.workspace()
        .list_documents(None, Some(document_id))
        .await
        .into_iter()
        .next()
        .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
    authorize_workspace_notebook_str(state.auth(), permission, &document.workspace_id)
}

pub(crate) async fn authorize_document_access_index_or_query(
    state: &AppState,
    document_id: &str,
) -> Result<(), AppError> {
    if !matches!(state.auth().subject_kind(), SubjectKind::ApiKey) {
        return Ok(());
    }
    let document = state.workspace()
        .list_documents(None, Some(document_id))
        .await
        .into_iter()
        .next()
        .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
    authorize_workspace_index_or_query_str(state.auth(), &document.workspace_id)
}

pub(crate) fn parse_workspace_id(value: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(value.trim()).map_err(|_| {
        AppError::validation("invalid_workspace_id", "workspace_id must be a valid UUID")
    })
}

pub(crate) fn require_workspace_id_arg(arguments: &serde_json::Value) -> Result<Uuid, AppError> {
    let workspace_id = arguments
        .get("workspace_id")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim();
    if workspace_id.is_empty() {
        return Err(AppError::validation(
            "workspace_id_required",
            "workspace tools require arguments.workspace_id",
        ));
    }
    parse_workspace_id(workspace_id)
}

pub(crate) fn account_create_permission() -> &'static str {
    PERM_WORKSPACE_CREATE
}

pub(crate) fn account_list_permission() -> &'static str {
    PERM_WORKSPACE_LIST
}

pub(crate) fn index_permission() -> &'static str {
    PERM_INDEX
}

pub(crate) fn query_permission() -> &'static str {
    PERM_QUERY
}

/// Canonical forbidden code when a signed-in user lacks workspace membership.
pub(crate) const WORKSPACE_ACCESS_REQUIRED: &str = "workspace_access_required";

/// Require the signed-in user to have share/workspace access (not `AccessLevel::None`).
/// Public error code: [`WORKSPACE_ACCESS_REQUIRED`].
pub(crate) async fn ensure_user_workspace_access(
    state: &AppState,
    workspace_id: &str,
) -> Result<(), AppError> {
    if !matches!(state.auth().subject_kind(), SubjectKind::User) {
        return Ok(());
    }
    if state.auth().ensure_permission(PERM_ADMIN).is_ok() {
        return Ok(());
    }
    if state.postgres_configured() && state.share_store().is_none() {
        return Err(AppError::internal("share access verification unavailable"));
    }
    if !state.postgres_configured() {
        return Ok(());
    }
    let access = state.share().check_access(workspace_id).await?;
    if access == avrag_share::AccessLevel::None {
        return Err(AppError::forbidden(
            WORKSPACE_ACCESS_REQUIRED,
            "workspace membership required to manage API keys for this workspace",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::auth_runtime::UserId;

    #[test]
    fn workspace_access_required_constant_matches_forbidden_code() {
        // Drives the same constant the production deny path uses (no hard-coded drift).
        let err = AppError::forbidden(
            WORKSPACE_ACCESS_REQUIRED,
            "workspace membership required to manage API keys for this workspace",
        );
        assert_eq!(err.code(), WORKSPACE_ACCESS_REQUIRED);
        assert_eq!(err.code(), "workspace_access_required");
    }

    #[test]
    fn workspace_key_rejected_for_account_tools() {
        let auth = AuthContext::new(UserId::from(Uuid::new_v4()), SubjectKind::ApiKey)
            .with_workspace_scope(Uuid::new_v4())
            .grant(PERM_WORKSPACE_CREATE);
        let err = authorize_account_tool(&auth, PERM_WORKSPACE_CREATE).unwrap_err();
        assert_eq!(err.code(), "workspace_key_cannot_call_account_tools");
    }

    #[test]
    fn account_key_rejected_for_workspace_tools() {
        let auth =
            AuthContext::new(UserId::from(Uuid::new_v4()), SubjectKind::ApiKey).grant(PERM_QUERY);
        let err = authorize_workspace_tool(&auth, PERM_QUERY, Uuid::new_v4()).unwrap_err();
        assert_eq!(err.code(), "account_key_cannot_call_workspace_tools");
    }

    #[test]
    fn workspace_scope_mismatch_is_forbidden() {
        let scoped = Uuid::new_v4();
        let other = Uuid::new_v4();
        let auth = AuthContext::new(UserId::from(Uuid::new_v4()), SubjectKind::ApiKey)
            .with_workspace_scope(scoped)
            .grant(PERM_QUERY);
        let err = authorize_workspace_tool(&auth, PERM_QUERY, other).unwrap_err();
        assert_eq!(err.code(), "workspace_scope_mismatch");
    }
}
