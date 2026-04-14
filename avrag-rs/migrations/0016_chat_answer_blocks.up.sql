ALTER TABLE chat_messages
ADD COLUMN IF NOT EXISTS answer_blocks JSONB NOT NULL DEFAULT '[]'::jsonb;
