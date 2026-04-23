CREATE TABLE IF NOT EXISTS document_parse_runs (
    run_id UUID PRIMARY KEY,
    org_id UUID NOT NULL,
    notebook_id UUID NOT NULL,
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    status VARCHAR(32) NOT NULL,
    backend_summary JSONB NOT NULL DEFAULT '{}'::jsonb,
    duration_ms BIGINT,
    warnings_json JSONB NOT NULL DEFAULT '[]'::jsonb,
    error_json JSONB,
    artifact_path TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_document_parse_runs_org_id
    ON document_parse_runs(org_id);

CREATE INDEX IF NOT EXISTS idx_document_parse_runs_document_id
    ON document_parse_runs(document_id, created_at DESC);
