-- Add stripe_customer_id to users
ALTER TABLE users ADD COLUMN IF NOT EXISTS stripe_customer_id TEXT UNIQUE;

-- Remove stripe_customer_id from organizations
ALTER TABLE organizations DROP COLUMN IF EXISTS stripe_customer_id CASCADE;

-- Drop old subscriptions policies and indexes
DROP POLICY IF EXISTS tenant_isolation_subscriptions ON subscriptions;
DROP INDEX IF EXISTS idx_subscriptions_org;

-- Truncate subscriptions to avoid conflicts during migration
TRUNCATE TABLE subscriptions CASCADE;

-- Update subscriptions table: change from org_id to user_id
ALTER TABLE subscriptions DROP COLUMN IF EXISTS org_id CASCADE;
ALTER TABLE subscriptions ADD COLUMN user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE;

-- Unique constraint so that each user has at most one subscription
CREATE UNIQUE INDEX IF NOT EXISTS idx_subscriptions_user_unique ON subscriptions(user_id);
CREATE INDEX IF NOT EXISTS idx_subscriptions_user ON subscriptions(user_id);

-- Update RLS policies to restrict subscription access by user_id
CREATE POLICY user_isolation_subscriptions ON subscriptions
    USING (user_id = NULLIF(current_setting('app.current_user', true), '')::uuid);

-- Update quota limits: change enterprise to plus, and upgrade pro to unlimited
DELETE FROM quota_limits WHERE plan_id = 'enterprise';

INSERT INTO quota_limits (plan_id, metric_type, soft_limit, hard_limit) VALUES
    ('plus', 'pages_processed', 5000, 10000),
    ('plus', 'embedding_tokens', 5000000, 10000000),
    ('plus', 'llm_input_tokens', 500000, 1000000),
    ('plus', 'llm_output_tokens', 250000, 500000),
    ('plus', 'storage_bytes', 5368709120, 10737418240)
ON CONFLICT (plan_id, metric_type) DO UPDATE
SET soft_limit = EXCLUDED.soft_limit, hard_limit = EXCLUDED.hard_limit;

UPDATE quota_limits
SET soft_limit = NULL, hard_limit = NULL
WHERE plan_id = 'pro';
