-- Backfill deny-by-default owner policies on the seven tenant tables that had
-- no RLS policy at all when 0082 forced RLS on every owner_user_id table.
-- Under FORCE RLS a table without any policy is invisible to the runtime role
-- (default deny), which would silently break its access path; these policies
-- restore the same owner / admin GUC contract the other tenant tables use.
--
-- Coverage notes:
--   rag_text_chunks / rag_multimodal_chunks / rag_kg_* / rag_graph_passages —
--     pgvector retrieval option (RETRIEVAL_BACKEND=pgvector); the Milvus
--     default never touches them, but the tables exist either way.
--   osv7_share_links — share-link lookup; only reached with
--     app.public_share_token set by the share path.
--   provider_secret_audit — audit trail written on secret ops; owner- AND
--     ops_admin-readable.

SELECT set_config('app.current_role', 'super_admin', true);

CREATE OR REPLACE FUNCTION _mig0083_tenant_policy(tbl text, owner_is_text boolean, with_share boolean)
RETURNS void LANGUAGE plpgsql AS $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_policy WHERE polrelid = tbl::regclass) THEN
    IF owner_is_text THEN
      -- osv7_share_links predates the uuid owner axis: owner_user_id is text.
      EXECUTE format(
        'CREATE POLICY tenant_isolation_%I ON %I FOR ALL TO PUBLIC USING ('
        ' owner_user_id = NULLIF(current_setting(''app.current_user'', true), '''')'
        ' OR current_setting(''app.current_role'', true) = ANY (ARRAY[''super_admin''::text, ''admin''::text])'
        ' OR (%L::boolean AND current_setting(''app.public_share_token'', true) IS NOT NULL))', tbl, tbl, with_share);
    ELSE
      EXECUTE format(
        'CREATE POLICY tenant_isolation_%I ON %I FOR ALL TO PUBLIC USING ('
        ' owner_user_id = NULLIF(current_setting(''app.current_user'', true), '''')::uuid'
        ' OR current_setting(''app.current_role'', true) = ANY (ARRAY[''super_admin''::text, ''admin''::text])'
        ' OR (%L::boolean AND current_setting(''app.public_share_token'', true) IS NOT NULL))', tbl, tbl, with_share);
    END IF;
  END IF;
END $$;

SELECT _mig0083_tenant_policy('rag_text_chunks', false, false);
SELECT _mig0083_tenant_policy('rag_multimodal_chunks', false, false);
SELECT _mig0083_tenant_policy('rag_kg_entities', false, false);
SELECT _mig0083_tenant_policy('rag_kg_relations', false, false);
SELECT _mig0083_tenant_policy('rag_graph_passages', false, false);
SELECT _mig0083_tenant_policy('provider_secret_audit', false, false);
SELECT _mig0083_tenant_policy('osv7_share_links', true, true);

DROP FUNCTION _mig0083_tenant_policy(text, boolean, boolean);

SELECT '0083 no-policy backfill OK' AS result;