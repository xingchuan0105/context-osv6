CREATE TABLE IF NOT EXISTS message_tags (
    message_id BIGINT NOT NULL REFERENCES chat_messages(id) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (message_id, tag)
);

CREATE INDEX IF NOT EXISTS idx_message_tags_tag ON message_tags(tag);
CREATE INDEX IF NOT EXISTS idx_message_tags_message_id ON message_tags(message_id);

DROP INDEX IF EXISTS idx_chat_sessions_notebook_user;
DROP INDEX IF EXISTS idx_chat_messages_search_vector;

ALTER TABLE chat_messages DROP COLUMN IF EXISTS search_vector;
ALTER TABLE chat_messages DROP COLUMN IF EXISTS search_tokens;
