use contracts::auth_runtime::AuthContext;

/// Account owner id used for RLS / `owner_user_id`.
pub fn current_owner_user_id(auth: &AuthContext) -> String {
    auth.user_id().to_string()
}

/// Resolve the current actor user id from an auth context, falling back to the default user.
pub fn current_user_id(auth: &AuthContext) -> String {
    auth.actor_id()
        .map(|actor_id| actor_id.into_uuid().to_string())
        .unwrap_or_else(common::default_user_id)
}
