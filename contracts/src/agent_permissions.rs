//! Permission strings for workspace/org API keys and MCP tool authorization.

pub const PERM_QUERY: &str = "query";
pub const PERM_INDEX: &str = "index";
pub const PERM_ADMIN: &str = "admin";
pub const PERM_WORKSPACE_CREATE: &str = "workspace.create";
pub const PERM_WORKSPACE_LIST: &str = "workspace.list";

pub const ORG_KEY_DEFAULT_PERMISSIONS: &[&str] = &[PERM_WORKSPACE_CREATE, PERM_WORKSPACE_LIST];
pub const WORKSPACE_KEY_DEFAULT_PERMISSIONS: &[&str] = &[PERM_INDEX, PERM_QUERY];

/// Org creator / administrator role stored on `users.role`.
pub const USER_ROLE_ORG_ADMIN: &str = "org_admin";

pub fn user_role_grants_org_admin(role: &str) -> bool {
    matches!(role, USER_ROLE_ORG_ADMIN | "super_admin")
}

fn is_allowed_permission(permission: &str) -> bool {
    matches!(
        permission,
        PERM_QUERY | PERM_INDEX | PERM_ADMIN | PERM_WORKSPACE_CREATE | PERM_WORKSPACE_LIST
    )
}

fn is_workspace_permission(permission: &str) -> bool {
    matches!(permission, PERM_INDEX | PERM_QUERY)
}

fn is_org_permission(permission: &str) -> bool {
    matches!(permission, PERM_WORKSPACE_CREATE | PERM_WORKSPACE_LIST)
}

/// Normalize API key permissions at create time and on validate/read.
pub fn normalize_api_key_permissions(
    permissions: &[String],
    notebook_id: Option<uuid::Uuid>,
) -> Vec<String> {
    let mut normalized = if permissions.is_empty() {
        if notebook_id.is_some() {
            WORKSPACE_KEY_DEFAULT_PERMISSIONS
                .iter()
                .map(|value| (*value).to_string())
                .collect()
        } else {
            ORG_KEY_DEFAULT_PERMISSIONS
                .iter()
                .map(|value| (*value).to_string())
                .collect()
        }
    } else {
        permissions
            .iter()
            .map(|item| item.trim().to_lowercase())
            .filter(|item| is_allowed_permission(item))
            .collect::<Vec<_>>()
    };
    normalized.sort();
    normalized.dedup();
    if notebook_id.is_some() {
        normalized.retain(|item| is_workspace_permission(item));
    } else {
        normalized.retain(|item| is_org_permission(item));
    }
    if normalized.is_empty() {
        if notebook_id.is_some() {
            normalized.extend(
                WORKSPACE_KEY_DEFAULT_PERMISSIONS
                    .iter()
                    .map(|value| (*value).to_string()),
            );
        } else {
            normalized.extend(
                ORG_KEY_DEFAULT_PERMISSIONS
                    .iter()
                    .map(|value| (*value).to_string()),
            );
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn org_permissions_are_preserved() {
        let perms = normalize_api_key_permissions(
            &[
                PERM_WORKSPACE_CREATE.to_string(),
                PERM_WORKSPACE_LIST.to_string(),
            ],
            None,
        );
        assert!(perms.contains(&PERM_WORKSPACE_CREATE.to_string()));
        assert!(perms.contains(&PERM_WORKSPACE_LIST.to_string()));
    }

    #[test]
    fn org_permissions_strip_admin_and_workspace_scoped_values() {
        let perms = normalize_api_key_permissions(
            &[
                PERM_ADMIN.to_string(),
                PERM_INDEX.to_string(),
                PERM_WORKSPACE_CREATE.to_string(),
            ],
            None,
        );
        assert!(!perms.contains(&PERM_ADMIN.to_string()));
        assert!(!perms.contains(&PERM_INDEX.to_string()));
        assert_eq!(perms, vec![PERM_WORKSPACE_CREATE.to_string()]);
    }

    #[test]
    fn workspace_defaults_include_index_and_query() {
        let perms = normalize_api_key_permissions(&[], Some(Uuid::new_v4()));
        assert!(perms.contains(&PERM_INDEX.to_string()));
        assert!(perms.contains(&PERM_QUERY.to_string()));
    }

    #[test]
    fn workspace_permissions_strip_admin_and_org_scoped_values() {
        let perms = normalize_api_key_permissions(
            &[
                PERM_ADMIN.to_string(),
                PERM_INDEX.to_string(),
                PERM_WORKSPACE_CREATE.to_string(),
            ],
            Some(Uuid::new_v4()),
        );
        assert!(!perms.contains(&PERM_ADMIN.to_string()));
        assert!(!perms.contains(&PERM_WORKSPACE_CREATE.to_string()));
        assert_eq!(perms, vec![PERM_INDEX.to_string()]);
    }

    #[test]
    fn workspace_permissions_keep_index_and_query_only() {
        let perms = normalize_api_key_permissions(
            &[
                PERM_QUERY.to_string(),
                PERM_INDEX.to_string(),
                PERM_ADMIN.to_string(),
            ],
            Some(Uuid::new_v4()),
        );
        assert_eq!(perms, vec![PERM_INDEX.to_string(), PERM_QUERY.to_string()]);
    }
}
