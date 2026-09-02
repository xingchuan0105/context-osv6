-- Best-effort rollback. Session-bound artifacts (no workspace binding) cannot
-- exist under the restored NOT NULL world, so they are deleted — same precedent
-- as 0084.down deleting null-workspace sessions.

DELETE FROM document_assets WHERE workspace_id IS NULL;
DELETE FROM document_parse_runs WHERE workspace_id IS NULL;
DELETE FROM document_blocks WHERE workspace_id IS NULL;
DELETE FROM ingestion_tasks WHERE workspace_id IS NULL;
DELETE FROM document_cleanup_tasks WHERE workspace_id IS NULL;

DELETE FROM document_toc WHERE workspace_id IS NULL;

ALTER TABLE document_assets ALTER COLUMN workspace_id SET NOT NULL;
ALTER TABLE document_parse_runs ALTER COLUMN workspace_id SET NOT NULL;
ALTER TABLE document_blocks ALTER COLUMN workspace_id SET NOT NULL;
ALTER TABLE document_toc ALTER COLUMN workspace_id SET NOT NULL;
ALTER TABLE ingestion_tasks ALTER COLUMN workspace_id SET NOT NULL;
ALTER TABLE document_cleanup_tasks ALTER COLUMN workspace_id SET NOT NULL;

ALTER TABLE document_toc
    ADD CONSTRAINT document_toc_workspace_id_fkey
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE;
ALTER TABLE ingestion_tasks
    ADD CONSTRAINT ingestion_tasks_workspace_id_fkey
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE;
ALTER TABLE document_cleanup_tasks
    ADD CONSTRAINT document_cleanup_tasks_workspace_id_fkey
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE;

ALTER TABLE documents ADD COLUMN workspace_id UUID;
UPDATE documents d
SET workspace_id = b.workspace_id
FROM (
    SELECT artifact_id, MIN(workspace_id::text)::uuid AS workspace_id
    FROM workspace_document_bindings
    GROUP BY artifact_id
) b
WHERE b.artifact_id = d.id;
DELETE FROM documents WHERE workspace_id IS NULL;
ALTER TABLE documents ALTER COLUMN workspace_id SET NOT NULL;
ALTER TABLE documents
    ADD CONSTRAINT documents_workspace_id_fkey
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE;
