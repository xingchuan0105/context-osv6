-- Conversation memory: jieba-tokenized FTS on user messages; drop unused tag table.

ALTER TABLE chat_messages
ADD COLUMN IF NOT EXISTS search_tokens TEXT;

ALTER TABLE chat_messages
ADD COLUMN IF NOT EXISTS search_vector tsvector
GENERATED ALWAYS AS (to_tsvector('simple', coalesce(search_tokens, ''))) STORED;

CREATE INDEX IF NOT EXISTS idx_chat_messages_search_vector
    ON chat_messages
    USING GIN (search_vector);

CREATE INDEX IF NOT EXISTS idx_chat_sessions_notebook_user
    ON chat_sessions (notebook_id, user_id);

-- Interim backfill until rows are re-segmented by the app on new writes.
UPDATE chat_messages
SET search_tokens = trim(coalesce(content, '') || ' ' || coalesce(resolved_query, ''))
WHERE search_tokens IS NULL;

DROP INDEX IF EXISTS idx_message_tags_message_id;
DROP INDEX IF EXISTS idx_message_tags_tag;
DROP TABLE IF EXISTS message_tags;
