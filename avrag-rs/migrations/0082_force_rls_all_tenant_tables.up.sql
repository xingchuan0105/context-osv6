-- Hardening: force RLS on every public table that carries owner_user_id.
-- Defense in depth: FORCE makes even the table owner (role `avrag`) subject to
-- the policies, so an accidentally privileged runtime connection cannot read
-- across tenants as long as app.current_user is not set to the victim.

SELECT set_config('app.current_role', 'super_admin', true);

DO $$
DECLARE
  r RECORD;
BEGIN
  FOR r IN
    SELECT c.relname AS table_name
    FROM pg_class c
    JOIN pg_namespace n ON n.oid = c.relnamespace
    WHERE n.nspname = 'public'
      AND c.relkind = 'r'
      AND EXISTS (
        SELECT 1 FROM pg_attribute a
        WHERE a.attrelid = c.oid
          AND a.attname = 'owner_user_id'
          AND NOT a.attisdropped
      )
      AND NOT EXISTS (
        -- _org_owner_map_mig is migration scratch state, not tenant data
        SELECT 1 FROM pg_attribute a2
        WHERE a2.attrelid = c.oid AND a2.attname = '__scratch'
      )
  LOOP
    EXECUTE format('ALTER TABLE public.%I ENABLE ROW LEVEL SECURITY', r.table_name);
    EXECUTE format('ALTER TABLE public.%I FORCE ROW LEVEL SECURITY', r.table_name);
  END LOOP;
END $$;

-- _org_owner_map_mig is a migration-internal scratch table; pin it down too so
-- nothing user-reachable ever reads it through the runtime role (grants in the
-- provisioning SQL simply never cover it).
ALTER TABLE _org_owner_map_mig ENABLE ROW LEVEL SECURITY;
ALTER TABLE _org_owner_map_mig FORCE ROW LEVEL SECURITY;