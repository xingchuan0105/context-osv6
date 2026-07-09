-- Reverse 0055: workspace → notebook naming.

DO $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM pg_policies WHERE schemaname = 'public' AND tablename = 'workspaces' AND policyname = 'tenant_isolation_workspaces'
  ) THEN
    ALTER POLICY tenant_isolation_workspaces ON workspaces RENAME TO tenant_isolation_notebooks;
  END IF;
  IF EXISTS (
    SELECT 1 FROM pg_policies WHERE schemaname = 'public' AND tablename = 'workspaces' AND policyname = 'admin_access_workspaces'
  ) THEN
    ALTER POLICY admin_access_workspaces ON workspaces RENAME TO admin_access_notebooks;
  END IF;
  IF EXISTS (
    SELECT 1 FROM pg_policies WHERE schemaname = 'public' AND tablename = 'workspace_members' AND policyname = 'tenant_isolation_workspace_members'
  ) THEN
    ALTER POLICY tenant_isolation_workspace_members ON workspace_members RENAME TO tenant_isolation_notebook_members;
  END IF;
  IF EXISTS (
    SELECT 1 FROM pg_policies WHERE schemaname = 'public' AND tablename = 'workspace_members' AND policyname = 'admin_access_workspace_members'
  ) THEN
    ALTER POLICY admin_access_workspace_members ON workspace_members RENAME TO admin_access_notebook_members;
  END IF;
END $$;

DO $$
DECLARE
  r RECORD;
  new_name text;
BEGIN
  FOR r IN
    SELECT indexname
    FROM pg_indexes
    WHERE schemaname = 'public'
      AND indexname LIKE '%workspace%'
      AND indexname LIKE '%workspace%' -- rename only those we flipped from notebook
  LOOP
    new_name := replace(r.indexname, 'workspace', 'notebook');
    -- avoid rewriting unrelated "workspace" indexes that never said notebook
    IF r.indexname LIKE '%workspace%' AND (
         r.indexname LIKE '%workspaces%'
      OR r.indexname LIKE '%workspace_id%'
      OR r.indexname LIKE '%workspace_members%'
      OR r.indexname LIKE '%org_workspace%'
    ) THEN
      IF new_name <> r.indexname AND NOT EXISTS (
        SELECT 1 FROM pg_indexes WHERE schemaname = 'public' AND indexname = new_name
      ) THEN
        EXECUTE format('ALTER INDEX %I RENAME TO %I', r.indexname, new_name);
      END IF;
    END IF;
  END LOOP;
END $$;

DO $$
DECLARE
  r RECORD;
BEGIN
  FOR r IN
    SELECT table_name
    FROM information_schema.columns
    WHERE table_schema = 'public'
      AND column_name = 'workspace_id'
  LOOP
    EXECUTE format(
      'ALTER TABLE %I RENAME COLUMN workspace_id TO notebook_id',
      r.table_name
    );
  END LOOP;
END $$;

ALTER TABLE IF EXISTS workspace_members RENAME TO notebook_members;
ALTER TABLE IF EXISTS workspaces RENAME TO notebooks;
