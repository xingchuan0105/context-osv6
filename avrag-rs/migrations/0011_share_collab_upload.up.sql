ALTER TABLE notebooks
    ADD COLUMN IF NOT EXISTS access_level TEXT NOT NULL DEFAULT 'private';

ALTER TABLE notebook_members
    ADD COLUMN IF NOT EXISTS id UUID DEFAULT gen_random_uuid(),
    ADD COLUMN IF NOT EXISTS email TEXT,
    ADD COLUMN IF NOT EXISTS invite_status TEXT NOT NULL DEFAULT 'accepted',
    ADD COLUMN IF NOT EXISTS invited_by UUID REFERENCES users(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS invited_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ADD COLUMN IF NOT EXISTS accepted_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

UPDATE notebook_members
SET id = COALESCE(id, gen_random_uuid()),
    invite_status = COALESCE(invite_status, 'accepted'),
    invited_by = COALESCE(invited_by, added_by),
    invited_at = COALESCE(invited_at, added_at),
    accepted_at = COALESCE(accepted_at, added_at),
    updated_at = COALESCE(updated_at, NOW())
WHERE TRUE;

ALTER TABLE notebook_members
    ALTER COLUMN id SET NOT NULL;

ALTER TABLE notebook_members
    DROP CONSTRAINT IF EXISTS notebook_members_pkey;

ALTER TABLE notebook_members
    ALTER COLUMN user_id DROP NOT NULL;

ALTER TABLE notebook_members
    ADD CONSTRAINT notebook_members_pkey PRIMARY KEY (id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_notebook_members_unique_user
    ON notebook_members(notebook_id, user_id)
    WHERE user_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_notebook_members_unique_email
    ON notebook_members(notebook_id, lower(email))
    WHERE email IS NOT NULL;

ALTER TABLE share_tokens
    ADD COLUMN IF NOT EXISTS access_count INTEGER NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS share_access_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    notebook_id UUID NOT NULL REFERENCES notebooks(id) ON DELETE CASCADE,
    share_token TEXT REFERENCES share_tokens(token) ON DELETE SET NULL,
    accessor_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    accessor_ip TEXT,
    action TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE share_access_logs ENABLE ROW LEVEL SECURITY;
ALTER TABLE share_access_logs FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation_share_access_logs ON share_access_logs
    USING (org_id = NULLIF(current_setting('app.current_org', true), '')::uuid);

CREATE POLICY admin_access_share_access_logs ON share_access_logs
    USING (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'));

CREATE INDEX IF NOT EXISTS idx_share_access_logs_org_notebook
    ON share_access_logs(org_id, notebook_id, created_at DESC);
