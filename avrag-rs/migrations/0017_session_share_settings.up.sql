ALTER TABLE chat_sessions
    ADD COLUMN IF NOT EXISTS pinned BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE notebooks
    ADD COLUMN IF NOT EXISTS allow_download BOOLEAN NOT NULL DEFAULT FALSE;

CREATE INDEX IF NOT EXISTS idx_chat_sessions_org_notebook_pinned_updated
    ON chat_sessions(org_id, notebook_id, pinned DESC, updated_at DESC);
