-- ADR-0010 PR7 §3.2: cloud BYOK provider secrets (envelope encryption at rest).
-- Ciphertext + nonce only; never plaintext. Scope: owner_user_id + optional workspace_id.

SELECT set_config('app.current_role', 'super_admin', true);

CREATE TABLE IF NOT EXISTS user_provider_secrets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- NULL = account-level default; non-null = workspace override.
    workspace_id UUID NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    -- llm | embedding | rerank
    purpose TEXT NOT NULL CHECK (purpose IN ('llm', 'embedding', 'rerank')),
    -- Provider id, e.g. deepseek | openai | siliconflow | ...
    provider TEXT NOT NULL,
    base_url TEXT NULL,
    model_hint TEXT NULL,
    -- AES-256-GCM ciphertext of the API key (never plaintext).
    ciphertext BYTEA NOT NULL,
    -- 12-byte GCM nonce for this secret.
    nonce BYTEA NOT NULL,
    -- Display-only fingerprint: last4:length (e.g. "xYz1:51"). Never full key.
    key_fingerprint TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at TIMESTAMPTZ NULL
);

-- One active secret per (owner, account-default scope, purpose).
CREATE UNIQUE INDEX IF NOT EXISTS uq_user_provider_secrets_account_active
    ON user_provider_secrets (owner_user_id, purpose)
    WHERE revoked_at IS NULL AND workspace_id IS NULL;

-- One active secret per (owner, workspace, purpose).
CREATE UNIQUE INDEX IF NOT EXISTS uq_user_provider_secrets_workspace_active
    ON user_provider_secrets (owner_user_id, workspace_id, purpose)
    WHERE revoked_at IS NULL AND workspace_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_user_provider_secrets_owner
    ON user_provider_secrets (owner_user_id)
    WHERE revoked_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_user_provider_secrets_owner_ws
    ON user_provider_secrets (owner_user_id, workspace_id)
    WHERE revoked_at IS NULL;

ALTER TABLE user_provider_secrets ENABLE ROW LEVEL SECURITY;
ALTER TABLE user_provider_secrets FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS user_isolation_user_provider_secrets ON user_provider_secrets;
CREATE POLICY user_isolation_user_provider_secrets ON user_provider_secrets
    USING (
        owner_user_id = NULLIF(current_setting('app.current_user', true), '')::uuid
        OR current_setting('app.current_role', true) IN ('super_admin', 'admin', 'ops_admin', 'finance_admin')
    )
    WITH CHECK (
        owner_user_id = NULLIF(current_setting('app.current_user', true), '')::uuid
        OR current_setting('app.current_role', true) IN ('super_admin', 'admin', 'ops_admin', 'finance_admin')
    );
