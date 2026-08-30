-- ============================================================================
-- avrag-rs runtime role provisioning — idempotent, versioned operator SQL.
--
-- Three-layer role model (docs/engineering/2026-08-30-tenant-isolation-p0):
--   avrag_cluster_admin  SUPERUSER            role/ext/DR only; never deployed
--   avrag                migration runner     object owner, NOSUPERUSER, NOBYPASSRLS
--   avrag_runtime         API/worker DML only  no owns, NOBYPASSRLS, no member-of
--
-- Run as a cluster-admin session targeting the app database:
--   psql "$MIGRATION_DATABASE_URL" -f avrag-rs/db/roles/002_runtime_grants.sql
-- Never run against the runtime DSN (CREATE ROLE requires cluster privilege).
-- ============================================================================
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

ALTER ROLE avrag NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE;
ALTER ROLE avrag NOINHERIT;

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
GRANT CONNECT ON DATABASE CURRENT_DATABASE() TO avrag_runtime;
GRANT USAGE ON SCHEMA public TO avrag_runtime;
REVOKE CREATE ON SCHEMA public FROM PUBLIC;
REVOKE CREATE ON SCHEMA public FROM avrag_runtime;
REVOKE ALL ON DATABASE CURRENT_DATABASE() FROM PUBLIC;

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
    t := r.table_name;
    CONTINUE WHEN t = '_sqlx_migrations';
    CONTINUE WHEN t LIKE '_org_owner_map%';
    EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON public.%I TO avrag_runtime', t);
  END LOOP;
END $$;

-- Sequences (serial columns; identity columns get USAGE implicitly but be
-- generous: gen_random_uuid defaults do not need them, plain serials do).
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

-- Functions: execute on user-callable helpers (segment etc.); never grant on
-- anything marked privileged. Grant blanket EXECUTE minus clearly internal ones.
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
ALTER DATABASE CURRENT_DATABASE() OWNER TO avrag_cluster_admin;

-- ── 6) assertions: fail loudly if the runtime role is not safe ---------------
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
  SELECT string_agg(table_name, ', ')
    INTO own_record
  FROM information_schema.tables
  WHERE table_schema = 'public'
    AND table_type = 'BASE TABLE'
    AND tableowner NOT IN ('avrag');

  IF own_record IS NOT NULL THEN
    RAISE EXCEPTION 'tables not owned by avrag (must be re-owned or dropped): %', own_record;
  END IF;
END $$;

SELECT 'runtime role grants OK' AS result;