-- Alter subscriptions table: make stripe columns nullable
ALTER TABLE subscriptions ALTER COLUMN stripe_subscription_id DROP NOT NULL;
ALTER TABLE subscriptions ALTER COLUMN stripe_price_id DROP NOT NULL;

-- Add billing_provider and unified provider columns
ALTER TABLE subscriptions ADD COLUMN IF NOT EXISTS billing_provider TEXT NOT NULL DEFAULT 'stripe' CHECK (billing_provider IN ('stripe', 'creem', 'alipay'));
ALTER TABLE subscriptions ADD COLUMN IF NOT EXISTS provider_subscription_id TEXT;
ALTER TABLE subscriptions ADD COLUMN IF NOT EXISTS provider_price_id TEXT;

-- Populate existing stripe subscription columns to the unified provider columns
UPDATE subscriptions
SET provider_subscription_id = stripe_subscription_id,
    provider_price_id = stripe_price_id
WHERE provider_subscription_id IS NULL;

-- Create partial unique index and expiry index
CREATE UNIQUE INDEX IF NOT EXISTS idx_subscriptions_provider_sub_unique 
ON subscriptions(billing_provider, provider_subscription_id) 
WHERE provider_subscription_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_subscriptions_expiry_scan 
ON subscriptions(status, current_period_end) 
WHERE status = 'active';

-- Rename webhook events table
ALTER TABLE stripe_webhook_events RENAME TO webhook_events;

-- Alter webhook events: add provider, claimed_at, lease_expires_at, and processed_at
ALTER TABLE webhook_events ADD COLUMN IF NOT EXISTS provider TEXT NOT NULL DEFAULT 'stripe' CHECK (provider IN ('stripe', 'creem', 'alipay'));
ALTER TABLE webhook_events ADD COLUMN IF NOT EXISTS claimed_at TIMESTAMPTZ;
ALTER TABLE webhook_events ADD COLUMN IF NOT EXISTS lease_expires_at TIMESTAMPTZ;
ALTER TABLE webhook_events ADD COLUMN IF NOT EXISTS processed_at TIMESTAMPTZ;

-- Drop old unique constraint on event_id if exists and add new unique constraint on (provider, event_id)
ALTER TABLE webhook_events DROP CONSTRAINT IF EXISTS stripe_webhook_events_event_id_key;
DROP INDEX IF EXISTS stripe_webhook_events_event_id_key;
ALTER TABLE webhook_events ADD CONSTRAINT webhook_events_provider_event_id_key UNIQUE (provider, event_id);

-- Create billing_orders table
CREATE TABLE IF NOT EXISTS billing_orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider TEXT NOT NULL CHECK (provider IN ('stripe', 'creem', 'alipay')),
    provider_order_id TEXT,
    plan_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'paid', 'failed', 'refunded', 'canceled')),
    amount_cents INT NOT NULL,
    currency TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Enable RLS and add isolation policies for billing_orders
ALTER TABLE billing_orders ENABLE ROW LEVEL SECURITY;
ALTER TABLE billing_orders FORCE ROW LEVEL SECURITY;

CREATE POLICY user_isolation_orders ON billing_orders
    USING (user_id = NULLIF(current_setting('app.current_user', true), '')::uuid);

CREATE POLICY admin_access_orders ON billing_orders
    USING (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'));

CREATE INDEX IF NOT EXISTS idx_billing_orders_user ON billing_orders(user_id);
CREATE INDEX IF NOT EXISTS idx_billing_orders_provider_order ON billing_orders(provider, provider_order_id);

-- Create billing_outbox table
CREATE TABLE IF NOT EXISTS billing_outbox (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'sent', 'failed')),
    retry_count INT NOT NULL DEFAULT 0,
    dedupe_key TEXT UNIQUE NOT NULL,
    error TEXT,
    processed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_billing_outbox_status ON billing_outbox(status);
