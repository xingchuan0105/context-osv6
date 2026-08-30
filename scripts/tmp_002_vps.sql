-- VPS variant of 002_runtime_grants.sql: the object owner role 'avrag' here is
-- the old bootstrap superuser, already demoted out-of-band (single-user catalog
-- edit). Cluster admin + runtime role creation are run by 'avrag' before its
-- own password is rotated; the two ALTER ROLE avrag lines are kept for
-- idempotency on fresh hosts.
--
-- Parameters:
--   :runtime_password   password of avrag_runtime (psql -v runtime_password=...)

-- ── 1) cluster admin (create only if missing; never rotate here) ------------
SELECT format(
    'CREATE ROLE avrag_cluster_admin LOGIN SUPERUSER PASSWORD %L',
    'set-by-operator'
) WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'avrag_cluster_admin') \gexec

-- ── 2) migration/owner role (create only if missing) ------------------------
SELECT format(
    'CREATE ROLE avrag LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE PASSWORD %L',
    'set-by-operator'
) WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'avrag') \gexec

-- (avrag demotion already applied on hosts where it was bootstrap-superuser)

-- ── 3) runtime role — hardened ----------------------------------------------
SELECT format(
    'CREATE ROLE avrag_runtime LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE PASSWORD %L',
    :'runtime_password'
) WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'avrag_runtime') \gexec

ALTER ROLE avrag_runtime NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE NOINHERIT;
ALTER ROLE avrag_runtime CONNECTION LIMIT 100;
ALTER ROLE avrag_runtime SET statement_timeout = '60s';
ALTER ROLE avrag_runtime SET idle_in_transaction_session_timeout = '30s';
ALTER ROLE avrag_runtime SET lock_timeout = '10s';
ALTER ROLE avrag_runtime SET default_transaction_read_only = off;
ALTER ROLE avrag_runtime SET search_path = public;

-- ── 4) grants ---------------------------------------------------------------
-- CURRENT_DATABASE() cannot be used as an identifier in GRANT/ALTER statements;
-- the maintenance runbook targets the single product DB on this host.
GRANT CONNECT ON DATABASE avrag_rs TO avrag_runtime;
GRANT USAGE ON SCHEMA public TO avrag_runtime;
REVOKE CREATE ON SCHEMA public FROM PUBLIC;
REVOKE CREATE ON SCHEMA public FROM avrag_runtime;
REVOKE ALL ON DATABASE avrag_rs FROM PUBLIC;

-- DML on business tables: everything except migration-ledger/sensitive sets.
DO $$
DECLARE
  r RECORD;
  t text;
BEGIN
  FOR r IN
    SELECT c.relname AS table_name
    FROM pg_class c
    JOIN pg_namespace n ON n.oid = c.relnamespace
    WHERE n.nspname = 'public' AND c.relkind = 'r'
  LOOP
    CONTINUE WHEN r.table_name = '_sqlx_migrations';
    CONTINUE WHEN r.table_name LIKE '_org_owner_map%';
    EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON public.%I TO avrag_runtime', r.table_name);
  END LOOP;
END $$;

-- Sequences
DO $$
DECLARE
  r RECORD;
BEGIN
  FOR r IN
    SELECT sequencename FROM pg_sequences WHERE schemaname = 'public'
  LOOP
    EXECUTE format('GRANT USAGE, SELECT ON SEQUENCE public.%I TO avrag_runtime', r.sequencename);
  END LOOP;
END $$;

-- Functions: blanket execute in public schema.
DO $$
DECLARE
  r RECORD;
BEGIN
  FOR r IN
    SELECT p.oid::regprocedure AS fn
    FROM pg_proc p
    JOIN pg_namespace n ON n.oid = p.pronamespace
    WHERE n.nspname = 'public'
      AND p.prokind = 'f'
  LOOP
    EXECUTE format('GRANT EXECUTE ON FUNCTION %s TO avrag_runtime', r.fn);
  END LOOP;
END $$;

-- ── 5) future tables created by migrations (owner = avrag) auto-granted -----
ALTER DEFAULT PRIVILEGES FOR ROLE avrag IN SCHEMA public
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO avrag_runtime;
ALTER DEFAULT PRIVILEGES FOR ROLE avrag IN SCHEMA public
    GRANT USAGE, SELECT ON SEQUENCES TO avrag_runtime;
ALTER DEFAULT PRIVILEGES FOR ROLE avrag IN SCHEMA public
    GRANT USAGE ON TYPES TO avrag_runtime;

ALTER SCHEMA public OWNER TO avrag_cluster_admin;
ALTER DATABASE avrag_rs OWNER TO avrag_cluster_admin;

-- ── 6) assertions -----------------------------------------------------------
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'avrag_runtime')
     AND (
       (SELECT rolsuper   FROM pg_roles WHERE rolname = 'avrag_runtime')
       OR (SELECT rolbypassrls FROM pg_roles WHERE rolname = 'avrag_runtime')
       OR (SELECT rolcreatedb  FROM pg_roles WHERE rolname = 'avrag_runtime')
       OR (SELECT rolcreaterole FROM pg_roles WHERE rolname = 'avrag_runtime')
       OR (SELECT rolinherit   FROM pg_roles WHERE rolname = 'avrag_runtime')
       OR EXISTS (
            SELECT 1 FROM pg_auth_members m
            WHERE m.member = (SELECT oid FROM pg_roles WHERE rolname = 'avrag_runtime')
       )
     ) THEN
    RAISE EXCEPTION 'avrag_runtime must not hold superuser/bypassrls/createdb/createrole/inherit or any role membership';
  END IF;
END $$;

DO $$
DECLARE
  own_record text;
BEGIN
  SELECT string_agg(c.relname, ', ')
    INTO own_record
  FROM pg_class c
  JOIN pg_namespace n ON n.oid = c.relnamespace
  WHERE n.nspname = 'public'
    AND c.relkind = 'r'
    AND (SELECT relname <> '_sqlx_migrations' AND c.relname NOT LIKE '_org_owner_map%')
    AND c.relowner <> (SELECT oid FROM pg_roles WHERE rolname = 'avrag');

  IF own_record IS NOT NULL THEN
    RAISE EXCEPTION 'tables not owned by avrag (must be re-owned or dropped): %', own_record;
  END IF;
END $$;

SELECT 'runtime role grants OK' AS result;