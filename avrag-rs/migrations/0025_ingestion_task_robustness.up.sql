ALTER TABLE ingestion_tasks
    ADD COLUMN IF NOT EXISTS max_attempts INTEGER NOT NULL DEFAULT 5,
    ADD COLUMN IF NOT EXISTS dead_lettered_at TIMESTAMPTZ NULL,
    ADD COLUMN IF NOT EXISTS last_failed_at TIMESTAMPTZ NULL,
    ADD COLUMN IF NOT EXISTS lock_token UUID NULL;

UPDATE ingestion_tasks
SET attempt_count = 0
WHERE attempt_count < 0;

UPDATE ingestion_tasks
SET max_attempts = 5
WHERE max_attempts <= 0;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'ingestion_tasks_attempt_count_nonnegative'
    ) THEN
        ALTER TABLE ingestion_tasks
            ADD CONSTRAINT ingestion_tasks_attempt_count_nonnegative CHECK (attempt_count >= 0);
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'ingestion_tasks_max_attempts_positive'
    ) THEN
        ALTER TABLE ingestion_tasks
            ADD CONSTRAINT ingestion_tasks_max_attempts_positive CHECK (max_attempts > 0);
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_ingestion_tasks_processing_stale
    ON ingestion_tasks(locked_at, enqueued_at)
    WHERE status = 'processing' AND dead_lettered_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_ingestion_tasks_dead_lettered
    ON ingestion_tasks(dead_lettered_at DESC, updated_at DESC)
    WHERE dead_lettered_at IS NOT NULL OR status = 'dead_letter';
