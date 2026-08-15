-- 2026-08-15 W2 (desktop cloud login / official-key relay): desktop relay tokens.
-- Long-lived, revocable, user-scoped credentials authorizing ONLY /v1/relay/*.
-- Token format `cos_dt_<32 hex>`; only the sha256 hash is stored.

SELECT set_config('app.current_role', 'super_admin', true);

CREATE TABLE IF NOT EXISTS desktop_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- Display label, e.g. device name ("MacBook Pro").
    name TEXT NOT NULL,
    -- sha256(plaintext token), hex. Never store plaintext.
    token_hash TEXT NOT NULL UNIQUE,
    -- Display-only prefix (e.g. "cos_dt_ab12cd"). Never full token.
    prefix TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ NULL,
    revoked_at TIMESTAMPTZ NULL
);

CREATE INDEX IF NOT EXISTS idx_desktop_tokens_owner
    ON desktop_tokens (owner_user_id)
    WHERE revoked_at IS NULL;

ALTER TABLE desktop_tokens ENABLE ROW LEVEL SECURITY;
ALTER TABLE desktop_tokens FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS user_isolation_desktop_tokens ON desktop_tokens;
CREATE POLICY user_isolation_desktop_tokens ON desktop_tokens
    USING (
        owner_user_id = NULLIF(current_setting('app.current_user', true), '')::uuid
        OR current_setting('app.current_role', true) IN ('super_admin', 'admin', 'ops_admin', 'finance_admin')
    )
    WITH CHECK (
        owner_user_id = NULLIF(current_setting('app.current_user', true), '')::uuid
        OR current_setting('app.current_role', true) IN ('super_admin', 'admin', 'ops_admin', 'finance_admin')
    );
