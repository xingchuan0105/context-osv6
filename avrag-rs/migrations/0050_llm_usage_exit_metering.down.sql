DROP INDEX IF EXISTS idx_llm_usage_user_kind_time;
ALTER TABLE llm_usage_events DROP COLUMN IF EXISTS usage_kind;
