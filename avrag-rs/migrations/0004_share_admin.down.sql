DROP INDEX IF EXISTS idx_share_tokens_active;
DROP INDEX IF EXISTS idx_share_tokens_org_notebook;
DROP INDEX IF EXISTS idx_notebook_members_org_notebook;

DROP POLICY IF EXISTS tenant_isolation_share_tokens ON share_tokens;
DROP POLICY IF EXISTS tenant_isolation_notebook_members ON notebook_members;

DROP TABLE IF EXISTS share_tokens;
DROP TABLE IF EXISTS notebook_members;

ALTER TABLE users DROP COLUMN IF EXISTS role;
ALTER TABLE organizations DROP COLUMN IF EXISTS blocked;
