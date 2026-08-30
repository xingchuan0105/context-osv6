-- No destructive undo: drop only the policies this migration created, but keep
-- the tables' ENABLE/FORCE RLS state from 0082.
DO $$
DECLARE
  r RECORD;
BEGIN
  FOR r IN
    SELECT polname, polrelid::regclass AS tablename
    FROM pg_policy
    WHERE polname LIKE 'tenant_isolation_%'
      AND polrelid::regclass::text IN (
        'rag_text_chunks', 'rag_multimodal_chunks', 'rag_kg_entities',
        'rag_kg_relations', 'rag_graph_passages', 'provider_secret_audit',
        'osv7_share_links'
      )
      AND (SELECT count(*) FROM pg_policy p WHERE p.polrelid = pg_policy.polrelid) = 1
  LOOP
    EXECUTE format('DROP POLICY IF EXISTS %I ON %I', r.polname, r.tablename);
  END LOOP;
END $$;