ALTER TABLE chat_sessions
    ALTER COLUMN workspace_id DROP NOT NULL;

ALTER TABLE chat_sessions
    ADD COLUMN model_role TEXT NOT NULL DEFAULT 'agent'
    CONSTRAINT chat_sessions_model_role_check
    CHECK (model_role IN ('agent', 'quick_chat'));
