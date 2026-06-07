ALTER TABLE organizations ADD COLUMN IF NOT EXISTS stripe_customer_id TEXT UNIQUE;
ALTER TABLE users DROP COLUMN IF EXISTS stripe_customer_id CASCADE;

DROP POLICY IF EXISTS user_isolation_subscriptions ON subscriptions;
DROP INDEX IF EXISTS idx_subscriptions_user_unique;
DROP INDEX IF EXISTS idx_subscriptions_user;

TRUNCATE TABLE subscriptions CASCADE;

ALTER TABLE subscriptions DROP COLUMN IF EXISTS user_id CASCADE;
ALTER TABLE subscriptions ADD COLUMN org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE;

CREATE INDEX IF NOT EXISTS idx_subscriptions_org ON subscriptions(org_id);

CREATE POLICY tenant_isolation_subscriptions ON subscriptions
    USING (org_id = NULLIF(current_setting('app.current_org', true), '')::uuid);

DELETE FROM quota_limits WHERE plan_id = 'plus';

-- Restore original limits: pro gets limited, enterprise gets restored
UPDATE quota_limits
SET soft_limit = 5000, hard_limit = 10000
WHERE plan_id = 'pro' AND metric_type = 'pages_processed';

UPDATE quota_limits
SET soft_limit = 5000000, hard_limit = 10000000
WHERE plan_id = 'pro' AND metric_type = 'embedding_tokens';

UPDATE quota_limits
SET soft_limit = 500000, hard_limit = 1000000
WHERE plan_id = 'pro' AND metric_type = 'llm_input_tokens';

UPDATE quota_limits
SET soft_limit = 250000, hard_limit = 500000
WHERE plan_id = 'pro' AND metric_type = 'llm_output_tokens';

UPDATE quota_limits
SET soft_limit = 5368709120, hard_limit = 10737418240
WHERE plan_id = 'pro' AND metric_type = 'storage_bytes';

INSERT INTO quota_limits (plan_id, metric_type, soft_limit, hard_limit) VALUES
    ('enterprise', 'pages_processed', NULL, NULL),
    ('enterprise', 'embedding_tokens', NULL, NULL),
    ('enterprise', 'llm_input_tokens', NULL, NULL),
    ('enterprise', 'llm_output_tokens', NULL, NULL),
    ('enterprise', 'storage_bytes', NULL, NULL)
ON CONFLICT (plan_id, metric_type) DO NOTHING;
