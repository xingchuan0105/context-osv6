ALTER TABLE document_assets ENABLE ROW LEVEL SECURITY;
ALTER TABLE document_assets FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation_document_assets ON document_assets;
CREATE POLICY tenant_isolation_document_assets ON document_assets
    USING (org_id = NULLIF(current_setting('app.current_org', true), '')::uuid)
    WITH CHECK (org_id = NULLIF(current_setting('app.current_org', true), '')::uuid);

ALTER TABLE document_multimodal_chunks ENABLE ROW LEVEL SECURITY;
ALTER TABLE document_multimodal_chunks FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation_document_multimodal_chunks ON document_multimodal_chunks;
CREATE POLICY tenant_isolation_document_multimodal_chunks ON document_multimodal_chunks
    USING (org_id = NULLIF(current_setting('app.current_org', true), '')::uuid)
    WITH CHECK (org_id = NULLIF(current_setting('app.current_org', true), '')::uuid);

ALTER TABLE document_parse_runs ENABLE ROW LEVEL SECURITY;
ALTER TABLE document_parse_runs FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation_document_parse_runs ON document_parse_runs;
CREATE POLICY tenant_isolation_document_parse_runs ON document_parse_runs
    USING (org_id = NULLIF(current_setting('app.current_org', true), '')::uuid)
    WITH CHECK (org_id = NULLIF(current_setting('app.current_org', true), '')::uuid);

ALTER TABLE document_blocks ENABLE ROW LEVEL SECURITY;
ALTER TABLE document_blocks FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation_document_blocks ON document_blocks;
CREATE POLICY tenant_isolation_document_blocks ON document_blocks
    USING (org_id = NULLIF(current_setting('app.current_org', true), '')::uuid)
    WITH CHECK (org_id = NULLIF(current_setting('app.current_org', true), '')::uuid);
