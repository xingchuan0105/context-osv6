-- Drop billing_outbox table
DROP TABLE IF EXISTS billing_outbox;

-- Drop billing_orders table
DROP TABLE IF EXISTS billing_orders CASCADE;

-- Revert webhook_events changes
ALTER TABLE webhook_events DROP CONSTRAINT IF EXISTS webhook_events_provider_event_id_key;
ALTER TABLE webhook_events DROP COLUMN IF EXISTS provider;
ALTER TABLE webhook_events DROP COLUMN IF EXISTS claimed_at;
ALTER TABLE webhook_events DROP COLUMN IF EXISTS lease_expires_at;

-- Add back unique constraint on event_id for stripe_webhook_events
-- To ensure consistency, rename back first
ALTER TABLE webhook_events RENAME TO stripe_webhook_events;
ALTER TABLE stripe_webhook_events ADD CONSTRAINT stripe_webhook_events_event_id_key UNIQUE (event_id);

-- Revert subscriptions changes
DROP INDEX IF EXISTS idx_subscriptions_expiry_scan;
DROP INDEX IF EXISTS idx_subscriptions_provider_sub_unique;

ALTER TABLE subscriptions DROP COLUMN IF EXISTS billing_provider;
ALTER TABLE subscriptions DROP COLUMN IF EXISTS provider_subscription_id;
ALTER TABLE subscriptions DROP COLUMN IF EXISTS provider_price_id;

-- Make stripe columns NOT NULL again (might fail if there are NULLs, but standard for clean rollback)
-- We can set a dummy value or clear invalid rows if needed, but standard SQL rollback is fine
ALTER TABLE subscriptions ALTER COLUMN stripe_subscription_id SET NOT NULL;
ALTER TABLE subscriptions ALTER COLUMN stripe_price_id SET NOT NULL;
