DROP POLICY IF EXISTS tenant_isolation_document_blocks ON document_blocks;
ALTER TABLE document_blocks NO FORCE ROW LEVEL SECURITY;
ALTER TABLE document_blocks DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_document_parse_runs ON document_parse_runs;
ALTER TABLE document_parse_runs NO FORCE ROW LEVEL SECURITY;
ALTER TABLE document_parse_runs DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_document_multimodal_chunks ON document_multimodal_chunks;
ALTER TABLE document_multimodal_chunks NO FORCE ROW LEVEL SECURITY;
ALTER TABLE document_multimodal_chunks DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_document_assets ON document_assets;
ALTER TABLE document_assets NO FORCE ROW LEVEL SECURITY;
ALTER TABLE document_assets DISABLE ROW LEVEL SECURITY;
