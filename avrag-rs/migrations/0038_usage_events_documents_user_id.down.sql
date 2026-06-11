DROP INDEX IF EXISTS idx_documents_user_id;
DROP INDEX IF EXISTS idx_usage_events_user_created_at;

ALTER TABLE documents DROP COLUMN IF EXISTS user_id;
ALTER TABLE usage_events DROP COLUMN IF EXISTS user_id;
