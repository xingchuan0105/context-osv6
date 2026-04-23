CREATE TABLE IF NOT EXISTS document_blocks (
    row_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL,
    notebook_id UUID NOT NULL,
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    block_id TEXT NOT NULL,
    page INTEGER,
    block_type VARCHAR(32) NOT NULL,
    modality VARCHAR(32) NOT NULL,
    text TEXT NOT NULL,
    summary_text TEXT,
    caption TEXT,
    asset_refs JSONB NOT NULL DEFAULT '[]'::jsonb,
    section_path JSONB NOT NULL DEFAULT '[]'::jsonb,
    source_locator_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    parser_backend VARCHAR(32) NOT NULL,
    metadata_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_document_blocks_document_block_id
    ON document_blocks(document_id, block_id);

CREATE INDEX IF NOT EXISTS idx_document_blocks_org_id
    ON document_blocks(org_id);

CREATE INDEX IF NOT EXISTS idx_document_blocks_document_id
    ON document_blocks(document_id, page, created_at);
