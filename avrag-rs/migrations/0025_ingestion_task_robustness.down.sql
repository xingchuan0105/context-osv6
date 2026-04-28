DROP INDEX IF EXISTS idx_ingestion_tasks_dead_lettered;
DROP INDEX IF EXISTS idx_ingestion_tasks_processing_stale;

ALTER TABLE ingestion_tasks
    DROP CONSTRAINT IF EXISTS ingestion_tasks_max_attempts_positive,
    DROP CONSTRAINT IF EXISTS ingestion_tasks_attempt_count_nonnegative;

ALTER TABLE ingestion_tasks
    DROP COLUMN IF EXISTS lock_token,
    DROP COLUMN IF EXISTS last_failed_at,
    DROP COLUMN IF EXISTS dead_lettered_at,
    DROP COLUMN IF EXISTS max_attempts;
