DROP INDEX IF EXISTS idx_document_cleanup_tasks_dead_lettered;
DROP INDEX IF EXISTS idx_document_cleanup_tasks_org_document;
DROP INDEX IF EXISTS idx_document_cleanup_tasks_processing_stale;
DROP INDEX IF EXISTS idx_document_cleanup_tasks_status_available;

DROP TABLE IF EXISTS document_cleanup_tasks;

UPDATE documents
SET status = 'failed',
    updated_at = NOW()
WHERE status IN ('deleting', 'deleted');

ALTER TABLE documents
    DROP COLUMN IF EXISTS deletion_error,
    DROP COLUMN IF EXISTS deleted_at,
    DROP COLUMN IF EXISTS deletion_requested_at;
