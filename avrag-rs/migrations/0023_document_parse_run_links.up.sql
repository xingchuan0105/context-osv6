ALTER TABLE document_blocks
ADD COLUMN IF NOT EXISTS parse_run_id UUID REFERENCES document_parse_runs(run_id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_document_blocks_parse_run_id
    ON document_blocks(parse_run_id);

ALTER TABLE document_assets
ADD COLUMN IF NOT EXISTS parse_run_id UUID REFERENCES document_parse_runs(run_id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_document_assets_parse_run_id
    ON document_assets(parse_run_id);

ALTER TABLE document_multimodal_chunks
ADD COLUMN IF NOT EXISTS parse_run_id UUID REFERENCES document_parse_runs(run_id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_document_multimodal_chunks_parse_run_id
    ON document_multimodal_chunks(parse_run_id);

ALTER TABLE chunks
ADD COLUMN IF NOT EXISTS parse_run_id UUID REFERENCES document_parse_runs(run_id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_chunks_parse_run_id
    ON chunks(parse_run_id);
