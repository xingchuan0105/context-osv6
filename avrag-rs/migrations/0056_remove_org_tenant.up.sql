-- Product: remove org tenant axis (B2C personal account).
-- org_id columns → owner_user_id (maps to users.id); drop organizations;
-- RLS GUC app.current_org → app.current_user.

SELECT set_config('app.current_role', 'super_admin', true);
ALTER TABLE users DISABLE ROW LEVEL SECURITY;

-- ── 1) owner map: earliest user id per org ──────────────────────────────────
DROP TABLE IF EXISTS _org_owner_map_mig;
CREATE TABLE _org_owner_map_mig AS
SELECT org_id, (array_agg(id ORDER BY id))[1]::uuid AS owner_user_id
FROM users
GROUP BY org_id;

-- Orphan orgs (no users): synthesize a user row id = org_id, then map
INSERT INTO users (id, org_id, email, full_name, role)
SELECT o.id, o.id, o.id::text || '@orphan.local', 'Orphan account', 'user'
FROM organizations o
WHERE NOT EXISTS (SELECT 1 FROM users u WHERE u.org_id = o.id)
ON CONFLICT (id) DO NOTHING;

INSERT INTO _org_owner_map_mig (org_id, owner_user_id)
SELECT o.id, o.id
FROM organizations o
WHERE NOT EXISTS (SELECT 1 FROM _org_owner_map_mig m WHERE m.org_id = o.id);

-- ── 2) Drop ALL foreign keys that reference organizations ───────────────────
DO $$
DECLARE
  r RECORD;
BEGIN
  FOR r IN
    SELECT con.conname, rel.relname AS table_name
    FROM pg_constraint con
    JOIN pg_class rel ON rel.oid = con.conrelid
    JOIN pg_class frel ON frel.oid = con.confrelid
    JOIN pg_namespace n ON n.oid = rel.relnamespace
    WHERE con.contype = 'f'
      AND n.nspname = 'public'
      AND frel.relname = 'organizations'
  LOOP
    EXECUTE format('ALTER TABLE %I DROP CONSTRAINT %I', r.table_name, r.conname);
  END LOOP;
END $$;

-- ── 3) users: unique email globally, drop org_id ────────────────────────────
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_org_id_email_key;
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_org_id_fkey;
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'users_email_key'
  ) THEN
    ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE (email);
  END IF;
END $$;

-- Drop org-based policy before dropping the column
DROP POLICY IF EXISTS tenant_isolation_users ON users;
DROP POLICY IF EXISTS admin_access_users ON users;
ALTER TABLE users DROP COLUMN IF EXISTS org_id;

-- users RLS: self or super_admin (no org)
CREATE POLICY tenant_isolation_users ON users
  USING (
    id = NULLIF(current_setting('app.current_user', true), '')::uuid
    OR current_setting('app.current_role', true) IN ('super_admin', 'admin')
  )
  WITH CHECK (
    id = NULLIF(current_setting('app.current_user', true), '')::uuid
    OR current_setting('app.current_role', true) IN ('super_admin', 'admin')
  );
ALTER TABLE users ENABLE ROW LEVEL SECURITY;
ALTER TABLE users FORCE ROW LEVEL SECURITY;

-- ── 4) Remap + rename org_id → owner_user_id on all remaining tables ────────
DO $$
DECLARE
  r RECORD;
  has_fk boolean;
BEGIN
  FOR r IN
    SELECT c.relname AS table_name
    FROM pg_class c
    JOIN pg_namespace n ON n.oid = c.relnamespace
    JOIN pg_attribute a ON a.attrelid = c.oid
    WHERE n.nspname = 'public'
      AND c.relkind = 'r'
      AND a.attname = 'org_id'
      AND a.attnum > 0
      AND NOT a.attisdropped
      AND c.relname <> 'users'
      AND c.relname <> '_org_owner_map_mig'
    ORDER BY c.relname
  LOOP
    -- remap values to owner user ids
    EXECUTE format(
      'UPDATE %I t SET org_id = m.owner_user_id
       FROM _org_owner_map_mig m
       WHERE t.org_id = m.org_id',
      r.table_name
    );

    -- rename column
    EXECUTE format(
      'ALTER TABLE %I RENAME COLUMN org_id TO owner_user_id',
      r.table_name
    );

    -- FK to users(id)
    EXECUTE format(
      'ALTER TABLE %I
         ADD CONSTRAINT %I
         FOREIGN KEY (owner_user_id) REFERENCES users(id) ON DELETE CASCADE',
      r.table_name,
      r.table_name || '_owner_user_id_fkey'
    );
  END LOOP;
END $$;

-- ── 5) Drop organizations ───────────────────────────────────────────────────
DROP TABLE IF EXISTS organizations CASCADE;

-- ── 6) Recreate RLS policies on owner_user_id + app.current_user ────────────
DO $$
DECLARE
  r RECORD;
  pol text;
  tbl text;
BEGIN
  FOR r IN
    SELECT tablename, policyname
    FROM pg_policies
    WHERE schemaname = 'public'
      AND (
        qual::text LIKE '%current_org%'
        OR with_check::text LIKE '%current_org%'
        OR qual::text LIKE '%org_id%'
        OR with_check::text LIKE '%org_id%'
      )
  LOOP
    EXECUTE format('DROP POLICY IF EXISTS %I ON %I', r.policyname, r.tablename);
  END LOOP;

  -- Standard tenant isolation for tables that now have owner_user_id
  FOR tbl IN
    SELECT c.relname
    FROM pg_class c
    JOIN pg_namespace n ON n.oid = c.relnamespace
    JOIN pg_attribute a ON a.attrelid = c.oid
    WHERE n.nspname = 'public'
      AND c.relkind = 'r'
      AND a.attname = 'owner_user_id'
      AND a.attnum > 0
      AND NOT a.attisdropped
    ORDER BY c.relname
  LOOP
    pol := 'tenant_isolation_' || tbl;
    IF tbl = 'document_cleanup_tasks' THEN
      EXECUTE format(
        'CREATE POLICY %I ON %I
           USING (
             owner_user_id = NULLIF(current_setting(''app.current_user'', true), '''')::uuid
             OR current_setting(''app.document_cleanup_worker'', true) = ''true''
             OR current_setting(''app.current_role'', true) IN (''super_admin'', ''admin'')
           )
           WITH CHECK (
             owner_user_id = NULLIF(current_setting(''app.current_user'', true), '''')::uuid
             OR current_setting(''app.document_cleanup_worker'', true) = ''true''
             OR current_setting(''app.current_role'', true) IN (''super_admin'', ''admin'')
           )',
        pol, tbl
      );
    ELSE
      EXECUTE format(
        'CREATE POLICY %I ON %I
           USING (
             owner_user_id = NULLIF(current_setting(''app.current_user'', true), '''')::uuid
             OR current_setting(''app.current_role'', true) IN (''super_admin'', ''admin'')
           )
           WITH CHECK (
             owner_user_id = NULLIF(current_setting(''app.current_user'', true), '''')::uuid
             OR current_setting(''app.current_role'', true) IN (''super_admin'', ''admin'')
           )',
        pol, tbl
      );
    END IF;
  END LOOP;
END $$;

-- Rename indexes that still say org_id (cosmetic)
DO $$
DECLARE
  r RECORD;
  new_name text;
BEGIN
  FOR r IN
    SELECT indexname
    FROM pg_indexes
    WHERE schemaname = 'public'
      AND indexname LIKE '%org%'
  LOOP
    new_name := replace(replace(r.indexname, 'org_id', 'owner_user_id'), 'org', 'owner');
    -- keep short renames only when no collision
    IF new_name <> r.indexname
       AND length(new_name) < 63
       AND NOT EXISTS (
         SELECT 1 FROM pg_indexes WHERE schemaname = 'public' AND indexname = new_name
       )
    THEN
      BEGIN
        EXECUTE format('ALTER INDEX %I RENAME TO %I', r.indexname, new_name);
      EXCEPTION WHEN OTHERS THEN
        NULL;
      END;
    END IF;
  END LOOP;
END $$;
DROP TABLE IF EXISTS _org_owner_map_mig;
