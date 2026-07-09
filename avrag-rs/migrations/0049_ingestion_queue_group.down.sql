DROP INDEX IF EXISTS idx_ingestion_tasks_queue_group_status_enqueued;

ALTER TABLE ingestion_tasks
DROP COLUMN IF EXISTS queue_group;
