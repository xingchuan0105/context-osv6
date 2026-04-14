ALTER TABLE organizations
ADD COLUMN IF NOT EXISTS stripe_customer_id TEXT UNIQUE;

CREATE TABLE IF NOT EXISTS subscriptions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    stripe_subscription_id TEXT UNIQUE NOT NULL,
    stripe_price_id TEXT NOT NULL,
    plan_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    current_period_start TIMESTAMPTZ,
    current_period_end TIMESTAMPTZ,
    cancel_at_period_end BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS quota_limits (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plan_id TEXT NOT NULL,
    metric_type TEXT NOT NULL,
    soft_limit BIGINT,
    hard_limit BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(plan_id, metric_type)
);

CREATE TABLE IF NOT EXISTS stripe_webhook_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id TEXT NOT NULL UNIQUE,
    event_type TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'processing',
    payload JSONB,
    error TEXT,
    processed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE subscriptions ENABLE ROW LEVEL SECURITY;
ALTER TABLE subscriptions FORCE ROW LEVEL SECURITY;
ALTER TABLE quota_limits ENABLE ROW LEVEL SECURITY;
ALTER TABLE quota_limits FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation_subscriptions ON subscriptions
    USING (org_id = NULLIF(current_setting('app.current_org', true), '')::uuid);

CREATE POLICY admin_access_subscriptions ON subscriptions
    USING (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'));

CREATE POLICY quota_limits_public_read ON quota_limits
    USING (true);

CREATE POLICY quota_limits_public_insert ON quota_limits
    FOR INSERT
    WITH CHECK (true);

CREATE INDEX IF NOT EXISTS idx_subscriptions_org ON subscriptions(org_id);
CREATE INDEX IF NOT EXISTS idx_subscriptions_status ON subscriptions(status);
CREATE INDEX IF NOT EXISTS idx_stripe_webhook_events_type ON stripe_webhook_events(event_type);
CREATE INDEX IF NOT EXISTS idx_stripe_webhook_events_status ON stripe_webhook_events(status);
CREATE INDEX IF NOT EXISTS idx_stripe_webhook_events_created_at ON stripe_webhook_events(created_at DESC);

INSERT INTO quota_limits (plan_id, metric_type, soft_limit, hard_limit) VALUES
    ('free', 'pages_processed', 100, 500),
    ('free', 'embedding_tokens', 100000, 500000),
    ('free', 'llm_input_tokens', 50000, 100000),
    ('free', 'llm_output_tokens', 25000, 50000),
    ('free', 'storage_bytes', 1073741824, 5368709120),
    ('pro', 'pages_processed', 5000, 10000),
    ('pro', 'embedding_tokens', 5000000, 10000000),
    ('pro', 'llm_input_tokens', 500000, 1000000),
    ('pro', 'llm_output_tokens', 250000, 500000),
    ('pro', 'storage_bytes', 5368709120, 10737418240),
    ('enterprise', 'pages_processed', NULL, NULL),
    ('enterprise', 'embedding_tokens', NULL, NULL),
    ('enterprise', 'llm_input_tokens', NULL, NULL),
    ('enterprise', 'llm_output_tokens', NULL, NULL),
    ('enterprise', 'storage_bytes', NULL, NULL)
ON CONFLICT (plan_id, metric_type) DO NOTHING;
