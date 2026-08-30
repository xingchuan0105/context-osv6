-- Down: un-force is a no-op for isolation only; keeping policies but dropping
-- FORCE restores the pre-0082 state (owner exempt from RLS).

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
  LOOP
    EXECUTE format('DROP POLICY IF EXISTS __noop_0082 ON public.%I', r.table_name);
  END LOOP;
END $$;