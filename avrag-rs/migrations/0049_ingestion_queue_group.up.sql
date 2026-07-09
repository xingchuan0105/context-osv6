ALTER TABLE ingestion_tasks
ADD COLUMN IF NOT EXISTS queue_group TEXT NOT NULL DEFAULT 'default';

CREATE INDEX IF NOT EXISTS idx_ingestion_tasks_queue_group_status_enqueued
    ON ingestion_tasks(queue_group, status, enqueued_at);
