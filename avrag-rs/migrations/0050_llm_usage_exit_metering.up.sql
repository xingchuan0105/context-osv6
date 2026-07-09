-- Add usage_kind column to distinguish chat vs embedding calls
ALTER TABLE llm_usage_events
  ADD COLUMN IF NOT EXISTS usage_kind TEXT NOT NULL DEFAULT 'chat';

-- Index for monthly quota queries grouped by usage kind
CREATE INDEX IF NOT EXISTS idx_llm_usage_user_kind_time
  ON llm_usage_events(user_id, usage_kind, created_at DESC);
