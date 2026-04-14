CREATE TABLE IF NOT EXISTS api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    notebook_id UUID REFERENCES notebooks(id) ON DELETE CASCADE,
    key_hash TEXT NOT NULL UNIQUE,
    key_prefix TEXT NOT NULL,
    name TEXT NOT NULL,
    permissions TEXT[] NOT NULL DEFAULT ARRAY['query']::text[],
    rate_limit_rpm INTEGER NOT NULL DEFAULT 60,
    expires_at TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS user_profiles (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    expertise_domains JSONB NOT NULL DEFAULT '[]'::jsonb,
    preferred_answer_style TEXT,
    frequently_asked_topics JSONB NOT NULL DEFAULT '[]'::jsonb,
    custom_preferences JSONB NOT NULL DEFAULT '{}'::jsonb,
    inferred_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    inference_version TEXT NOT NULL DEFAULT 'v1',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS dialogue_states (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    session_id UUID NOT NULL UNIQUE REFERENCES chat_sessions(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    state_type TEXT NOT NULL DEFAULT 'working_memory',
    current_topic TEXT,
    pending_questions JSONB NOT NULL DEFAULT '[]'::jsonb,
    gathered_facts JSONB NOT NULL DEFAULT '[]'::jsonb,
    confidence_score REAL NOT NULL DEFAULT 0,
    state_history JSONB NOT NULL DEFAULT '[]'::jsonb,
    last_updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS notifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL DEFAULT '',
    data JSONB NOT NULL DEFAULT '{}'::jsonb,
    read_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS notification_outbox (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    notification_id UUID NOT NULL REFERENCES notifications(id) ON DELETE CASCADE,
    channel TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    attempts INTEGER NOT NULL DEFAULT 0,
    available_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    claimed_at TIMESTAMPTZ,
    delivered_at TIMESTAMPTZ,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE api_keys ENABLE ROW LEVEL SECURITY;
ALTER TABLE api_keys FORCE ROW LEVEL SECURITY;
ALTER TABLE user_profiles ENABLE ROW LEVEL SECURITY;
ALTER TABLE user_profiles FORCE ROW LEVEL SECURITY;
ALTER TABLE dialogue_states ENABLE ROW LEVEL SECURITY;
ALTER TABLE dialogue_states FORCE ROW LEVEL SECURITY;
ALTER TABLE notifications ENABLE ROW LEVEL SECURITY;
ALTER TABLE notifications FORCE ROW LEVEL SECURITY;
ALTER TABLE notification_outbox ENABLE ROW LEVEL SECURITY;
ALTER TABLE notification_outbox FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation_api_keys ON api_keys
    USING (org_id = NULLIF(current_setting('app.current_org', true), '')::uuid);
CREATE POLICY tenant_isolation_user_profiles ON user_profiles
    USING (org_id = NULLIF(current_setting('app.current_org', true), '')::uuid);
CREATE POLICY tenant_isolation_dialogue_states ON dialogue_states
    USING (org_id = NULLIF(current_setting('app.current_org', true), '')::uuid);
CREATE POLICY tenant_isolation_notifications ON notifications
    USING (org_id = NULLIF(current_setting('app.current_org', true), '')::uuid);
CREATE POLICY tenant_isolation_notification_outbox ON notification_outbox
    USING (org_id = NULLIF(current_setting('app.current_org', true), '')::uuid);

CREATE INDEX IF NOT EXISTS idx_api_keys_org_notebook ON api_keys(org_id, notebook_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_api_keys_key_hash ON api_keys(key_hash);
CREATE INDEX IF NOT EXISTS idx_user_profiles_org_user ON user_profiles(org_id, user_id);
CREATE INDEX IF NOT EXISTS idx_dialogue_states_org_session ON dialogue_states(org_id, session_id);
CREATE INDEX IF NOT EXISTS idx_notifications_user_created ON notifications(org_id, user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_notification_outbox_org_status ON notification_outbox(org_id, status, available_at);
