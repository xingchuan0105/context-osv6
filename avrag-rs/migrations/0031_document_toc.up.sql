CREATE TABLE IF NOT EXISTS document_toc (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    notebook_id UUID NOT NULL REFERENCES notebooks(id) ON DELETE CASCADE,
    parent_id UUID REFERENCES document_toc(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    heading_level INTEGER NOT NULL DEFAULT 1,
    page INTEGER,
    chunk_id UUID REFERENCES chunks(id) ON DELETE SET NULL,
    rank INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_document_toc_document_id ON document_toc(document_id);
CREATE INDEX IF NOT EXISTS idx_document_toc_parent_id ON document_toc(parent_id);
