-- T4b directed patch (run via cluster-admin psql session):
-- 1. provider_secret_audit is the only owner table in prod without a policy
--    (0082 forced RLS on it; no policy => invisible to runtime role).
-- 2. The other 0083 target tables (rag_*_chunks, rag_kg_*, rag_graph_passages,
--    osv7_share_links) do not exist in this deployment, so migration 0083 is
--    recorded as applied WITHOUT re-running it here.

SELECT set_config('app.current_role', 'super_admin', true);

CREATE POLICY tenant_isolation_provider_secret_audit ON provider_secret_audit FOR ALL TO PUBLIC USING (
  owner_user_id = NULLIF(current_setting('app.current_user', true), '')::uuid
  OR current_setting('app.current_role', true) = ANY (ARRAY['super_admin'::text, 'admin'::text])
);

INSERT INTO _sqlx_migrations (version, description, installed_on, success, checksum, execution_time)
VALUES (83, 'no policy tenant backfill', now(), true,
        decode('3ebcde1bd104c84e8a16f03c726f5aa635bd7490e775e39f21575f519c61ddec', 'hex'), 0)
ON CONFLICT (version) DO UPDATE SET success = true;

SELECT 't4b directed patch OK' AS result;