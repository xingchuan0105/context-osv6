-- ADR-0010 B3b: local workspace → cloud publish mapping (vector import, no re-ingest).

SELECT set_config('app.current_role', 'super_admin', true);

CREATE TABLE IF NOT EXISTS workspace_publish (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    cloud_workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    local_workspace_id UUID NOT NULL,
    upload_id UUID,
    status TEXT NOT NULL DEFAULT 'never'
        CHECK (status IN ('never', 'publishing', 'ready', 'failed')),
    embedding_model_id TEXT NOT NULL,
    vector_dim INT NOT NULL,
    expected_parts INT NOT NULL DEFAULT 0,
    last_published_at TIMESTAMPTZ,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (owner_user_id, local_workspace_id)
);

CREATE INDEX IF NOT EXISTS idx_workspace_publish_owner
    ON workspace_publish (owner_user_id);

CREATE INDEX IF NOT EXISTS idx_workspace_publish_upload
    ON workspace_publish (upload_id)
    WHERE upload_id IS NOT NULL;

ALTER TABLE workspace_publish ENABLE ROW LEVEL SECURITY;
ALTER TABLE workspace_publish FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS user_isolation_workspace_publish ON workspace_publish;
CREATE POLICY user_isolation_workspace_publish ON workspace_publish
    USING (
        owner_user_id = NULLIF(current_setting('app.current_user', true), '')::uuid
        OR current_setting('app.current_role', true) IN ('super_admin', 'admin', 'ops_admin', 'finance_admin')
    )
    WITH CHECK (
        owner_user_id = NULLIF(current_setting('app.current_user', true), '')::uuid
        OR current_setting('app.current_role', true) IN ('super_admin', 'admin', 'ops_admin', 'finance_admin')
    );
