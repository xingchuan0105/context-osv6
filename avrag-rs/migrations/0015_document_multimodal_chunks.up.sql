CREATE TABLE IF NOT EXISTS document_multimodal_chunks (
    chunk_id UUID PRIMARY KEY,
    org_id UUID NOT NULL,
    notebook_id UUID NOT NULL,
    document_id UUID NOT NULL,
    asset_id UUID REFERENCES document_assets(asset_id) ON DELETE CASCADE,
    page INTEGER,
    context_text TEXT,
    caption TEXT,
    normalized_text TEXT NOT NULL,
    parser_backend VARCHAR(32) NOT NULL,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_multimodal_chunks_org_id ON document_multimodal_chunks(org_id);
CREATE INDEX idx_multimodal_chunks_document_id ON document_multimodal_chunks(document_id);
CREATE INDEX idx_multimodal_chunks_asset_id ON document_multimodal_chunks(asset_id);
CREATE INDEX idx_multimodal_chunks_notebook_id ON document_multimodal_chunks(notebook_id);
