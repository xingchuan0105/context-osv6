-- Residual cleanup after org removal:
-- 1) drop leftover users.org_id (orgs table already gone)
-- 2) add users.blocked for admin account blocking (was organizations.blocked)

SELECT set_config('app.current_role', 'super_admin', true);

ALTER TABLE users DISABLE ROW LEVEL SECURITY;

ALTER TABLE users DROP CONSTRAINT IF EXISTS users_org_id_email_key;
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_org_id_fkey;
ALTER TABLE users DROP COLUMN IF EXISTS org_id;

ALTER TABLE users
  ADD COLUMN IF NOT EXISTS blocked BOOLEAN NOT NULL DEFAULT FALSE;

-- Ensure global unique email (idempotent)
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'users_email_key'
  ) THEN
    ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE (email);
  END IF;
END $$;

DROP POLICY IF EXISTS tenant_isolation_users ON users;
CREATE POLICY tenant_isolation_users ON users
  USING (
    id = NULLIF(current_setting('app.current_user', true), '')::uuid
    OR current_setting('app.current_role', true) IN ('super_admin', 'admin', 'ops_admin', 'finance_admin')
  )
  WITH CHECK (
    id = NULLIF(current_setting('app.current_user', true), '')::uuid
    OR current_setting('app.current_role', true) IN ('super_admin', 'admin', 'ops_admin', 'finance_admin')
  );

ALTER TABLE users ENABLE ROW LEVEL SECURITY;
ALTER TABLE users FORCE ROW LEVEL SECURITY;
