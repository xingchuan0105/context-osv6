use contracts::auth_runtime::AuthContext;

/// Resolve the current org id from an auth context.
pub fn current_org_id(auth: &AuthContext) -> String {
    auth.org_id().to_string()
}

/// Resolve the current user id from an auth context, falling back to the default user.
pub fn current_user_id(auth: &AuthContext) -> String {
    auth.actor_id()
        .map(|actor_id| actor_id.into_uuid().to_string())
        .unwrap_or_else(common::default_user_id)
}
