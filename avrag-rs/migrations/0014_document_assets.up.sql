CREATE TABLE IF NOT EXISTS document_assets (
    asset_id UUID PRIMARY KEY,
    org_id UUID NOT NULL,
    notebook_id UUID NOT NULL,
    document_id UUID NOT NULL,
    page INTEGER,
    asset_kind VARCHAR(32) NOT NULL,
    storage_path TEXT,
    mime_type VARCHAR(64),
    width INTEGER,
    height INTEGER,
    caption TEXT,
    parser_backend VARCHAR(32) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_document_assets_org_id ON document_assets(org_id);
CREATE INDEX idx_document_assets_document_id ON document_assets(document_id);
CREATE INDEX idx_document_assets_notebook_id ON document_assets(notebook_id);
