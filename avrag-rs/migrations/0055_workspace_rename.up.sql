-- Product decision 2026-07-09: full workspace naming (no notebook product surface).
-- Rename core table, members table, and all notebook_id columns → workspace_id.

-- 1) Core tables
ALTER TABLE IF EXISTS notebooks RENAME TO workspaces;
ALTER TABLE IF EXISTS notebook_members RENAME TO workspace_members;

-- 2) FK / data columns (IF EXISTS via DO blocks where needed for optional tables)
DO $$
DECLARE
  r RECORD;
BEGIN
  FOR r IN
    SELECT table_name
    FROM information_schema.columns
    WHERE table_schema = 'public'
      AND column_name = 'notebook_id'
  LOOP
    EXECUTE format(
      'ALTER TABLE %I RENAME COLUMN notebook_id TO workspace_id',
      r.table_name
    );
  END LOOP;
END $$;

-- 3) Indexes that still carry notebook in the name (best-effort renames)
DO $$
DECLARE
  r RECORD;
  new_name text;
BEGIN
  FOR r IN
    SELECT indexname
    FROM pg_indexes
    WHERE schemaname = 'public'
      AND indexname LIKE '%notebook%'
  LOOP
    new_name := replace(r.indexname, 'notebook', 'workspace');
    IF new_name <> r.indexname AND NOT EXISTS (
      SELECT 1 FROM pg_indexes WHERE schemaname = 'public' AND indexname = new_name
    ) THEN
      EXECUTE format('ALTER INDEX %I RENAME TO %I', r.indexname, new_name);
    END IF;
  END LOOP;
END $$;

-- 4) RLS policy names on renamed tables (cosmetic; table rename keeps policies attached)
DO $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM pg_policies WHERE schemaname = 'public' AND tablename = 'workspaces' AND policyname = 'tenant_isolation_notebooks'
  ) THEN
    ALTER POLICY tenant_isolation_notebooks ON workspaces RENAME TO tenant_isolation_workspaces;
  END IF;
  IF EXISTS (
    SELECT 1 FROM pg_policies WHERE schemaname = 'public' AND tablename = 'workspaces' AND policyname = 'admin_access_notebooks'
  ) THEN
    ALTER POLICY admin_access_notebooks ON workspaces RENAME TO admin_access_workspaces;
  END IF;
  IF EXISTS (
    SELECT 1 FROM pg_policies WHERE schemaname = 'public' AND tablename = 'workspace_members' AND policyname = 'tenant_isolation_notebook_members'
  ) THEN
    ALTER POLICY tenant_isolation_notebook_members ON workspace_members RENAME TO tenant_isolation_workspace_members;
  END IF;
  IF EXISTS (
    SELECT 1 FROM pg_policies WHERE schemaname = 'public' AND tablename = 'workspace_members' AND policyname = 'admin_access_notebook_members'
  ) THEN
    ALTER POLICY admin_access_notebook_members ON workspace_members RENAME TO admin_access_workspace_members;
  END IF;
END $$;
