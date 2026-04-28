ALTER TABLE documents
    ADD COLUMN IF NOT EXISTS deletion_requested_at TIMESTAMPTZ NULL,
    ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ NULL,
    ADD COLUMN IF NOT EXISTS deletion_error TEXT NULL;

CREATE TABLE IF NOT EXISTS document_cleanup_tasks (
    task_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    notebook_id UUID NOT NULL REFERENCES notebooks(id) ON DELETE CASCADE,
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    requested_by UUID REFERENCES users(id) ON DELETE SET NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    status TEXT NOT NULL DEFAULT 'queued',
    attempt_count INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 5,
    available_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    locked_at TIMESTAMPTZ NULL,
    locked_by TEXT NULL,
    lock_token UUID NULL,
    last_error TEXT NULL,
    last_failed_at TIMESTAMPTZ NULL,
    dead_lettered_at TIMESTAMPTZ NULL,
    completed_at TIMESTAMPTZ NULL,
    enqueued_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT document_cleanup_tasks_attempt_count_nonnegative CHECK (attempt_count >= 0),
    CONSTRAINT document_cleanup_tasks_max_attempts_positive CHECK (max_attempts > 0),
    CONSTRAINT document_cleanup_tasks_status_check CHECK (status IN ('queued', 'processing', 'completed', 'dead_letter'))
);

ALTER TABLE document_cleanup_tasks ENABLE ROW LEVEL SECURITY;
ALTER TABLE document_cleanup_tasks FORCE ROW LEVEL SECURITY;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_policies
        WHERE schemaname = current_schema()
          AND tablename = 'document_cleanup_tasks'
          AND policyname = 'tenant_isolation_document_cleanup_tasks'
    ) THEN
        CREATE POLICY tenant_isolation_document_cleanup_tasks ON document_cleanup_tasks
            USING (org_id = NULLIF(current_setting('app.current_org', true), '')::uuid);
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_document_cleanup_tasks_status_available
    ON document_cleanup_tasks(status, available_at, enqueued_at)
    WHERE status = 'queued';

CREATE INDEX IF NOT EXISTS idx_document_cleanup_tasks_processing_stale
    ON document_cleanup_tasks(locked_at, enqueued_at)
    WHERE status = 'processing' AND dead_lettered_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_document_cleanup_tasks_org_document
    ON document_cleanup_tasks(org_id, document_id);

CREATE INDEX IF NOT EXISTS idx_document_cleanup_tasks_dead_lettered
    ON document_cleanup_tasks(dead_lettered_at DESC, updated_at DESC)
    WHERE dead_lettered_at IS NOT NULL OR status = 'dead_letter';
