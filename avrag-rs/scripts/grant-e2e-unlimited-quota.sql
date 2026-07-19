-- Grant internal plan `e2e` (unlimited rolling usage) to the fixed
-- realistic-corpus identity used by product_e2e golden-set runs.
--
-- Semantics:
--   usage_limit_plan_policies.e2e = 0/0  → UsageLimitService treats 0 as unlimited
--   usage_limit_user_overrides     = 0/0  → highest precedence when both windows set
--   subscriptions.plan_id          = e2e  → get_user_plan; monthly quota_limits has no e2e rows
--                                           → hard_limit NULL → always allow
--
-- Product checkout never exposes `e2e` (API only free/plus/pro).
-- Does not change free-tier defaults for other users.
--
-- Usage (persistent smoke PG):
--   psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f scripts/grant-e2e-unlimited-quota.sql
-- Default URL: postgres://avrag:avrag@127.0.0.1:5432/avrag_rs_e2e_smoke
--
-- Also applied automatically at start of realistic_corpus_full_eval via
-- TestContext::grant_e2e_unlimited_quota.

SELECT set_config('app.current_role', 'super_admin', false);

INSERT INTO usage_limit_plan_policies
    (plan_id, rolling_5h_limit_units, rolling_7d_limit_units, enabled)
VALUES ('e2e', 0, 0, true)
ON CONFLICT (plan_id) DO UPDATE
SET rolling_5h_limit_units = 0,
    rolling_7d_limit_units = 0,
    enabled = true,
    updated_at = now();

INSERT INTO users (id, email)
VALUES (
  '00000000-0000-0000-0000-000000000001'::uuid,
  '00000000-0000-0000-0000-000000000001@local.dev'
)
ON CONFLICT (id) DO NOTHING;

INSERT INTO usage_limit_user_overrides
    (user_id, rolling_5h_limit_units, rolling_7d_limit_units, enabled)
VALUES (
  '00000000-0000-0000-0000-000000000001'::uuid,
  0, 0, true
)
ON CONFLICT (user_id) DO UPDATE
SET rolling_5h_limit_units = 0,
    rolling_7d_limit_units = 0,
    enabled = true,
    updated_at = now();

UPDATE subscriptions
SET plan_id = 'e2e',
    status = 'active',
    current_period_end = now() + interval '10 years',
    updated_at = now()
WHERE user_id = '00000000-0000-0000-0000-000000000001'::uuid
  AND status = 'active';

INSERT INTO subscriptions (
  user_id, plan_id, status, billing_provider,
  current_period_start, current_period_end, cancel_at_period_end
)
SELECT
  '00000000-0000-0000-0000-000000000001'::uuid,
  'e2e',
  'active',
  'creem',
  now() - interval '1 day',
  now() + interval '10 years',
  false
WHERE NOT EXISTS (
  SELECT 1 FROM subscriptions
  WHERE user_id = '00000000-0000-0000-0000-000000000001'::uuid
    AND status = 'active'
);

\echo === e2e unlimited grant ===
SELECT plan_id, rolling_5h_limit_units, rolling_7d_limit_units
FROM usage_limit_plan_policies WHERE plan_id = 'e2e';
SELECT user_id, rolling_5h_limit_units, rolling_7d_limit_units, enabled
FROM usage_limit_user_overrides
WHERE user_id = '00000000-0000-0000-0000-000000000001'::uuid;
SELECT user_id, plan_id, status, billing_provider
FROM subscriptions
WHERE user_id = '00000000-0000-0000-0000-000000000001'::uuid AND status = 'active';
