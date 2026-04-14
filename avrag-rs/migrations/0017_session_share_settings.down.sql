DROP INDEX IF EXISTS idx_chat_sessions_org_notebook_pinned_updated;

ALTER TABLE notebooks
    DROP COLUMN IF EXISTS allow_download;

ALTER TABLE chat_sessions
    DROP COLUMN IF EXISTS pinned;
