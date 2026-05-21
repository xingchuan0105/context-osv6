ALTER TABLE chat_messages ADD COLUMN IF NOT EXISTS tool_results JSONB NOT NULL DEFAULT '[]'::jsonb;
