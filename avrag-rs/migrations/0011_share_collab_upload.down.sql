DROP INDEX IF EXISTS idx_share_access_logs_org_notebook;
DROP POLICY IF EXISTS admin_access_share_access_logs ON share_access_logs;
DROP POLICY IF EXISTS tenant_isolation_share_access_logs ON share_access_logs;
DROP TABLE IF EXISTS share_access_logs;

ALTER TABLE share_tokens
    DROP COLUMN IF EXISTS access_count;

DROP INDEX IF EXISTS idx_notebook_members_unique_email;
DROP INDEX IF EXISTS idx_notebook_members_unique_user;

ALTER TABLE notebook_members
    DROP CONSTRAINT IF EXISTS notebook_members_pkey;

ALTER TABLE notebook_members
    ADD CONSTRAINT notebook_members_pkey PRIMARY KEY (notebook_id, user_id);

ALTER TABLE notebook_members
    ALTER COLUMN user_id SET NOT NULL;

ALTER TABLE notebook_members
    DROP COLUMN IF EXISTS updated_at,
    DROP COLUMN IF EXISTS accepted_at,
    DROP COLUMN IF EXISTS invited_at,
    DROP COLUMN IF EXISTS invited_by,
    DROP COLUMN IF EXISTS invite_status,
    DROP COLUMN IF EXISTS email,
    DROP COLUMN IF EXISTS id;

ALTER TABLE notebooks
    DROP COLUMN IF EXISTS access_level;
