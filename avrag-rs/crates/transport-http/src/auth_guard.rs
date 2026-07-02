use app_bootstrap::AppState;
use avrag_auth::{AuthContext, AuthError, SubjectKind};
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
        AuthError::NotebookScopeMismatch { expected, actual } => AppError::forbidden(
            "notebook_scope_mismatch",
            format!("API key is scoped to notebook {expected}, got {actual}"),
        ),
        AuthError::MissingNotebookScope => {
            AppError::forbidden("missing_notebook_scope", "workspace API key scope required")
        }
        AuthError::CrossTenantAccess => AppError::forbidden(
            "cross_tenant_access",
            "resource belongs to another organization",
        ),
        AuthError::MissingOrgScope => AppError::unauthorized("organization scope required"),
    }
}

fn ensure_permission_for_subject(auth: &AuthContext, permission: &str) -> Result<(), AppError> {
    if matches!(auth.subject_kind(), SubjectKind::User) {
        return Ok(());
    }
    auth.ensure_permission(permission)
        .map_err(auth_error_to_app_error)
}

pub(crate) fn authorize_org_tool(auth: &AuthContext, permission: &str) -> Result<(), AppError> {
    if auth.notebook_id().is_some() {
        return Err(AppError::forbidden(
            "workspace_key_cannot_call_org_tools",
            "workspace-scoped API keys cannot call org-level endpoints",
        ));
    }
    ensure_permission_for_subject(auth, permission)
}

pub(crate) fn authorize_workspace_tool(
    auth: &AuthContext,
    permission: &str,
    notebook_id: Uuid,
) -> Result<(), AppError> {
    if matches!(auth.subject_kind(), SubjectKind::ApiKey) {
        if auth.notebook_id().is_none() {
            return Err(AppError::forbidden(
                "org_key_cannot_call_workspace_tools",
                "org-scoped API keys cannot call workspace-level endpoints",
            ));
        }
        auth.ensure_notebook_scope(notebook_id)
            .map_err(auth_error_to_app_error)?;
    }
    ensure_permission_for_subject(auth, permission)
}

pub(crate) fn authorize_api_key_query_scoped(auth: &AuthContext) -> Result<(), AppError> {
    if !matches!(auth.subject_kind(), SubjectKind::ApiKey) {
        return Ok(());
    }
    ensure_permission_for_subject(auth, PERM_QUERY)?;
    if auth.notebook_id().is_none() {
        return Err(AppError::forbidden(
            "org_key_cannot_call_workspace_tools",
            "org-scoped API keys cannot call workspace-level endpoints",
        ));
    }
    Ok(())
}

pub(crate) fn authorize_workspace_index_or_query(
    auth: &AuthContext,
    notebook_id: Uuid,
) -> Result<(), AppError> {
    if matches!(auth.subject_kind(), SubjectKind::ApiKey) {
        if auth.notebook_id().is_none() {
            return Err(AppError::forbidden(
                "org_key_cannot_call_workspace_tools",
                "org-scoped API keys cannot call workspace-level endpoints",
            ));
        }
        auth.ensure_notebook_scope(notebook_id)
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
    notebook_id: &str,
) -> Result<(), AppError> {
    if !matches!(auth.subject_kind(), SubjectKind::ApiKey) {
        return Ok(());
    }
    let notebook_uuid = parse_notebook_id(notebook_id)?;
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
            "organization admin permission required",
        ));
    }
    auth.ensure_permission(PERM_ADMIN).map_err(|_| {
        AppError::forbidden("admin_required", "organization admin permission required")
    })
}

pub(crate) fn authorize_session_notebook(
    auth: &AuthContext,
    session_notebook_id: &str,
) -> Result<(), AppError> {
    if !matches!(auth.subject_kind(), SubjectKind::ApiKey) {
        return Ok(());
    }
    authorize_workspace_notebook_str(auth, query_permission(), session_notebook_id)
}

pub(crate) async fn authorize_session_access(
    state: &AppState,
    session_id: &str,
) -> Result<contracts::notebooks::ChatSession, AppError> {
    let session = state
        .get_session(session_id)
        .await
        .ok_or_else(|| AppError::not_found("session_not_found", "session not found"))?;
    authorize_session_notebook(state.auth(), &session.notebook_id)?;
    Ok(session)
}

pub(crate) async fn ensure_document_in_notebook(
    state: &AppState,
    document_id: &str,
    notebook_id: &str,
) -> Result<(), AppError> {
    let document = state
        .list_documents(None, Some(document_id))
        .await
        .into_iter()
        .next()
        .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
    if document.notebook_id != notebook_id {
        return Err(AppError::forbidden(
            "document_notebook_mismatch",
            "document does not belong to the requested workspace",
        ));
    }
    Ok(())
}

pub(crate) fn authorize_workspace_notebook_str(
    auth: &AuthContext,
    permission: &str,
    notebook_id: &str,
) -> Result<(), AppError> {
    if !matches!(auth.subject_kind(), SubjectKind::ApiKey) {
        return Ok(());
    }
    let notebook_uuid = parse_notebook_id(notebook_id)?;
    authorize_workspace_tool(auth, permission, notebook_uuid)
}

pub(crate) fn authorize_workspace_query_optional_notebook(
    auth: &AuthContext,
    notebook_id: Option<&str>,
) -> Result<(), AppError> {
    if !matches!(auth.subject_kind(), SubjectKind::ApiKey) {
        return Ok(());
    }
    let notebook_id = notebook_id
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            AppError::validation(
                "notebook_id_required",
                "notebook_id is required for workspace API keys",
            )
        })?;
    authorize_workspace_notebook_str(auth, PERM_QUERY, notebook_id)
}

pub(crate) async fn authorize_document_access(
    state: &AppState,
    document_id: &str,
    permission: &str,
) -> Result<(), AppError> {
    if !matches!(state.auth().subject_kind(), SubjectKind::ApiKey) {
        return Ok(());
    }
    let document = state
        .list_documents(None, Some(document_id))
        .await
        .into_iter()
        .next()
        .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
    authorize_workspace_notebook_str(state.auth(), permission, &document.notebook_id)
}

pub(crate) async fn authorize_document_access_index_or_query(
    state: &AppState,
    document_id: &str,
) -> Result<(), AppError> {
    if !matches!(state.auth().subject_kind(), SubjectKind::ApiKey) {
        return Ok(());
    }
    let document = state
        .list_documents(None, Some(document_id))
        .await
        .into_iter()
        .next()
        .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
    authorize_workspace_index_or_query_str(state.auth(), &document.notebook_id)
}

pub(crate) fn parse_notebook_id(value: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(value.trim()).map_err(|_| {
        AppError::validation("invalid_notebook_id", "notebook_id must be a valid UUID")
    })
}

pub(crate) fn require_notebook_id_arg(arguments: &serde_json::Value) -> Result<Uuid, AppError> {
    let notebook_id = arguments
        .get("notebook_id")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim();
    if notebook_id.is_empty() {
        return Err(AppError::validation(
            "notebook_id_required",
            "workspace tools require arguments.notebook_id",
        ));
    }
    parse_notebook_id(notebook_id)
}

pub(crate) fn org_create_permission() -> &'static str {
    PERM_WORKSPACE_CREATE
}

pub(crate) fn org_list_permission() -> &'static str {
    PERM_WORKSPACE_LIST
}

pub(crate) fn index_permission() -> &'static str {
    PERM_INDEX
}

pub(crate) fn query_permission() -> &'static str {
    PERM_QUERY
}

pub(crate) async fn ensure_user_notebook_access(
    state: &AppState,
    notebook_id: &str,
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
    let Some(store) = state.share_store() else {
        return Ok(());
    };
    let service = avrag_share::ShareService::new(store);
    let access = service
        .check_access(state.auth(), notebook_id)
        .await
        .map_err(|error| {
            AppError::internal(format!("failed to verify workspace access: {error}"))
        })?;
    if access == avrag_share::AccessLevel::None {
        return Err(AppError::forbidden(
            "notebook_access_required",
            "workspace membership required to manage API keys for this workspace",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use avrag_auth::OrgId;

    #[test]
    fn workspace_key_rejected_for_org_tools() {
        let auth = AuthContext::new(OrgId::from(Uuid::new_v4()), SubjectKind::ApiKey)
            .with_notebook_scope(Uuid::new_v4())
            .grant(PERM_WORKSPACE_CREATE);
        let err = authorize_org_tool(&auth, PERM_WORKSPACE_CREATE).unwrap_err();
        assert_eq!(err.code(), "workspace_key_cannot_call_org_tools");
    }

    #[test]
    fn org_key_rejected_for_workspace_tools() {
        let auth =
            AuthContext::new(OrgId::from(Uuid::new_v4()), SubjectKind::ApiKey).grant(PERM_QUERY);
        let err = authorize_workspace_tool(&auth, PERM_QUERY, Uuid::new_v4()).unwrap_err();
        assert_eq!(err.code(), "org_key_cannot_call_workspace_tools");
    }

    #[test]
    fn workspace_scope_mismatch_is_forbidden() {
        let scoped = Uuid::new_v4();
        let other = Uuid::new_v4();
        let auth = AuthContext::new(OrgId::from(Uuid::new_v4()), SubjectKind::ApiKey)
            .with_notebook_scope(scoped)
            .grant(PERM_QUERY);
        let err = authorize_workspace_tool(&auth, PERM_QUERY, other).unwrap_err();
        assert_eq!(err.code(), "notebook_scope_mismatch");
    }
}
