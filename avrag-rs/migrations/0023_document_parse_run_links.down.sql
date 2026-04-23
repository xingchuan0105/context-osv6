DROP INDEX IF EXISTS idx_chunks_parse_run_id;
ALTER TABLE chunks DROP COLUMN IF EXISTS parse_run_id;

DROP INDEX IF EXISTS idx_document_multimodal_chunks_parse_run_id;
ALTER TABLE document_multimodal_chunks DROP COLUMN IF EXISTS parse_run_id;

DROP INDEX IF EXISTS idx_document_assets_parse_run_id;
ALTER TABLE document_assets DROP COLUMN IF EXISTS parse_run_id;

DROP INDEX IF EXISTS idx_document_blocks_parse_run_id;
ALTER TABLE document_blocks DROP COLUMN IF EXISTS parse_run_id;
