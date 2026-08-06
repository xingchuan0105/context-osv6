-- Allow multiple LLM secrets (agent vs parse) distinguished by provider.
-- Old unique was (owner, purpose) only — one key per purpose account-wide.

SELECT set_config('app.current_role', 'super_admin', true);

DROP INDEX IF EXISTS uq_user_provider_secrets_account_active;
DROP INDEX IF EXISTS uq_user_provider_secrets_workspace_active;

CREATE UNIQUE INDEX IF NOT EXISTS uq_user_provider_secrets_account_purpose_provider
    ON user_provider_secrets (owner_user_id, purpose, provider)
    WHERE revoked_at IS NULL AND workspace_id IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS uq_user_provider_secrets_workspace_purpose_provider
    ON user_provider_secrets (owner_user_id, workspace_id, purpose, provider)
    WHERE revoked_at IS NULL AND workspace_id IS NOT NULL;
