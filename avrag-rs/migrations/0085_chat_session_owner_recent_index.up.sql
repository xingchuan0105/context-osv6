CREATE INDEX idx_chat_sessions_owner_recent
    ON chat_sessions (owner_user_id, updated_at DESC, created_at DESC);
