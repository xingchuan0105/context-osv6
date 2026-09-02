ALTER TABLE chat_sessions
    DROP CONSTRAINT chat_sessions_model_role_check;

ALTER TABLE chat_sessions
    DROP COLUMN model_role;

DELETE FROM chat_sessions
WHERE workspace_id IS NULL;

ALTER TABLE chat_sessions
    ALTER COLUMN workspace_id SET NOT NULL;
