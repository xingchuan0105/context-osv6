-- Chat-first W2a (second half of the wave): after code reads bindings only,
-- `documents.workspace_id` is deleted as the scope truth, and the derived /
-- queue tables stop pretending workspace is a NOT NULL fact. Session-bound
-- artifacts (chat-first W2b) have no workspace at all.
--
-- Queue tables keep a nullable workspace lineage column but lose the
-- FOREIGN KEY ... ON DELETE CASCADE: deleting a workspace must not delete the
-- cleanup task of an artifact that survives via a session binding (that would
-- leak the artifact past GC), nor kill in-flight ingestion for it.

ALTER TABLE documents DROP COLUMN workspace_id;

ALTER TABLE ingestion_tasks ALTER COLUMN workspace_id DROP NOT NULL;
ALTER TABLE document_cleanup_tasks ALTER COLUMN workspace_id DROP NOT NULL;

DO $mig0087_drop_workspace_fks$
DECLARE
    task_constraint RECORD;
BEGIN
    FOR task_constraint IN
        SELECT con.conname, con.conrelid::regclass AS tbl
        FROM pg_constraint con
        JOIN pg_attribute att
          ON att.attrelid = con.conrelid
         AND att.attname = 'workspace_id'
         AND con.conkey = ARRAY[att.attnum]
        WHERE con.contype = 'f'
          AND con.conrelid IN ('ingestion_tasks'::regclass, 'document_cleanup_tasks'::regclass)
    LOOP
        EXECUTE format('ALTER TABLE %s DROP CONSTRAINT %I', task_constraint.tbl, task_constraint.conname);
    END LOOP;
END
$mig0087_drop_workspace_fks$;

ALTER TABLE document_blocks ALTER COLUMN workspace_id DROP NOT NULL;
ALTER TABLE document_parse_runs ALTER COLUMN workspace_id DROP NOT NULL;
ALTER TABLE document_assets ALTER COLUMN workspace_id DROP NOT NULL;
ALTER TABLE document_toc ALTER COLUMN workspace_id DROP NOT NULL;

DO $mig0087_drop_toc_workspace_fk$
DECLARE
    toc_constraint RECORD;
BEGIN
    FOR toc_constraint IN
        SELECT con.conname, con.conrelid::regclass AS tbl
        FROM pg_constraint con
        JOIN pg_attribute att
          ON att.attrelid = con.conrelid
         AND att.attname = 'workspace_id'
         AND con.conkey = ARRAY[att.attnum]
        WHERE con.contype = 'f'
          AND con.conrelid = 'document_toc'::regclass
    LOOP
        EXECUTE format('ALTER TABLE %s DROP CONSTRAINT %I', toc_constraint.tbl, toc_constraint.conname);
    END LOOP;
END
$mig0087_drop_toc_workspace_fk$;
