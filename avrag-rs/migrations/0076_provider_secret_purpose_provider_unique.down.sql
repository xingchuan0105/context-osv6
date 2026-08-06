SELECT set_config('app.current_role', 'super_admin', true);

DROP INDEX IF EXISTS uq_user_provider_secrets_account_purpose_provider;
DROP INDEX IF EXISTS uq_user_provider_secrets_workspace_purpose_provider;

-- Restore pre-0076 uniqueness (one active secret per purpose). May fail if
-- multiple purpose+provider rows already exist — revoke extras first.
CREATE UNIQUE INDEX IF NOT EXISTS uq_user_provider_secrets_account_active
    ON user_provider_secrets (owner_user_id, purpose)
    WHERE revoked_at IS NULL AND workspace_id IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS uq_user_provider_secrets_workspace_active
    ON user_provider_secrets (owner_user_id, workspace_id, purpose)
    WHERE revoked_at IS NULL AND workspace_id IS NOT NULL;
