DROP INDEX IF EXISTS idx_llm_usage_user_billable_time;
ALTER TABLE llm_usage_events DROP COLUMN IF EXISTS billable;
