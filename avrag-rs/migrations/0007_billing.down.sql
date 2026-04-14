DROP INDEX IF EXISTS idx_stripe_webhook_events_created_at;
DROP INDEX IF EXISTS idx_stripe_webhook_events_status;
DROP INDEX IF EXISTS idx_stripe_webhook_events_type;
DROP INDEX IF EXISTS idx_subscriptions_status;
DROP INDEX IF EXISTS idx_subscriptions_org;

DROP POLICY IF EXISTS quota_limits_public_insert ON quota_limits;
DROP POLICY IF EXISTS quota_limits_public_read ON quota_limits;
DROP POLICY IF EXISTS admin_access_subscriptions ON subscriptions;
DROP POLICY IF EXISTS tenant_isolation_subscriptions ON subscriptions;

DROP TABLE IF EXISTS stripe_webhook_events;
DROP TABLE IF EXISTS quota_limits;
DROP TABLE IF EXISTS subscriptions;

ALTER TABLE organizations DROP COLUMN IF EXISTS stripe_customer_id;
