-- Chat-first W2a: typed document bindings (Artifact ↔ Conversation / Workspace).
-- `documents.workspace_id` stops being the scope truth; two tables with real
-- foreign keys replace it (design 2026-09-02-chat-first §4.3/§11.1). 0087 drops
-- the legacy column in the same wave — between the two, code reads bindings only.

CREATE TABLE IF NOT EXISTS conversation_document_bindings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    artifact_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    conversation_id UUID NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    owner_user_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT conversation_document_bindings_unique UNIQUE (conversation_id, artifact_id)
);

CREATE TABLE IF NOT EXISTS workspace_document_bindings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    artifact_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    owner_user_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT workspace_document_bindings_unique UNIQUE (workspace_id, artifact_id)
);

CREATE INDEX IF NOT EXISTS idx_conversation_document_bindings_artifact
    ON conversation_document_bindings (artifact_id);
CREATE INDEX IF NOT EXISTS idx_workspace_document_bindings_artifact
    ON workspace_document_bindings (artifact_id);

ALTER TABLE conversation_document_bindings ENABLE ROW LEVEL SECURITY;
ALTER TABLE conversation_document_bindings FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation_conversation_document_bindings
    ON conversation_document_bindings FOR ALL TO PUBLIC
    USING (
        owner_user_id = NULLIF(current_setting('app.current_user', true), '')::uuid
        OR current_setting('app.current_role', true) = ANY (ARRAY['super_admin'::text, 'admin'::text])
    );

ALTER TABLE workspace_document_bindings ENABLE ROW LEVEL SECURITY;
ALTER TABLE workspace_document_bindings FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation_workspace_document_bindings
    ON workspace_document_bindings FOR ALL TO PUBLIC
    USING (
        owner_user_id = NULLIF(current_setting('app.current_user', true), '')::uuid
        OR current_setting('app.current_role', true) = ANY (ARRAY['super_admin'::text, 'admin'::text])
    );

-- One-time truth migration: every workspace-scoped document becomes a binding.
INSERT INTO workspace_document_bindings (artifact_id, workspace_id, owner_user_id)
SELECT d.id, d.workspace_id, d.owner_user_id
FROM documents d
WHERE d.workspace_id IS NOT NULL
ON CONFLICT (workspace_id, artifact_id) DO NOTHING;
