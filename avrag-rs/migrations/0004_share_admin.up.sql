ALTER TABLE organizations
ADD COLUMN IF NOT EXISTS blocked BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE users
ADD COLUMN IF NOT EXISTS role TEXT NOT NULL DEFAULT 'user';

CREATE TABLE IF NOT EXISTS notebook_members (
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    notebook_id UUID NOT NULL REFERENCES notebooks(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    access_level TEXT NOT NULL,
    added_by UUID REFERENCES users(id) ON DELETE SET NULL,
    added_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (notebook_id, user_id)
);

CREATE TABLE IF NOT EXISTS share_tokens (
    token TEXT PRIMARY KEY,
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    notebook_id UUID NOT NULL REFERENCES notebooks(id) ON DELETE CASCADE,
    access_level TEXT NOT NULL,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ
);

ALTER TABLE notebook_members ENABLE ROW LEVEL SECURITY;
ALTER TABLE notebook_members FORCE ROW LEVEL SECURITY;
ALTER TABLE share_tokens ENABLE ROW LEVEL SECURITY;
ALTER TABLE share_tokens FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation_notebook_members ON notebook_members
    USING (org_id = NULLIF(current_setting('app.current_org', true), '')::uuid);

CREATE POLICY tenant_isolation_share_tokens ON share_tokens
    USING (org_id = NULLIF(current_setting('app.current_org', true), '')::uuid);

CREATE INDEX IF NOT EXISTS idx_notebook_members_org_notebook
    ON notebook_members(org_id, notebook_id);

CREATE INDEX IF NOT EXISTS idx_share_tokens_org_notebook
    ON share_tokens(org_id, notebook_id);

CREATE INDEX IF NOT EXISTS idx_share_tokens_active
    ON share_tokens(token, revoked_at, expires_at);
