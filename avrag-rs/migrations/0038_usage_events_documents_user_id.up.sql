-- B2C billing (0035) moved monthly quota accounting to user scope.
-- billing/core_usage.rs reads usage_events.user_id and documents.user_id.

ALTER TABLE usage_events
    ADD COLUMN IF NOT EXISTS user_id UUID REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE documents
    ADD COLUMN IF NOT EXISTS user_id UUID REFERENCES users(id) ON DELETE SET NULL;

-- Backfill historical rows: one user per org (B2C default) and notebook owner for documents.
UPDATE usage_events ue
SET user_id = sub.uid
FROM (
    SELECT DISTINCT ON (org_id) org_id, id AS uid
    FROM users
    ORDER BY org_id, created_at ASC
) sub
WHERE ue.org_id = sub.org_id
  AND ue.user_id IS NULL;

UPDATE documents d
SET user_id = n.owner_id
FROM notebooks n
WHERE d.notebook_id = n.id
  AND d.user_id IS NULL;

CREATE INDEX IF NOT EXISTS idx_usage_events_user_created_at
    ON usage_events(user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_documents_user_id
    ON documents(user_id);
