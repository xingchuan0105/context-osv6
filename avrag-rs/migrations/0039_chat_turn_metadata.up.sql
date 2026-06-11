ALTER TABLE chat_messages ADD COLUMN IF NOT EXISTS turn_metadata JSONB NOT NULL DEFAULT '{}'::jsonb;
