-- ADR-0010: usage_hold / release ledger kinds + retained markdown byte metric.

SELECT set_config('app.current_role', 'super_admin', true);

-- Expand wallet_ledger.kind check for atomic pre-debit holds.
ALTER TABLE wallet_ledger DROP CONSTRAINT IF EXISTS wallet_ledger_kind_check;
ALTER TABLE wallet_ledger ADD CONSTRAINT wallet_ledger_kind_check
    CHECK (kind IN (
        'signup_grant',
        'referral_bonus',
        'topup',
        'usage_debit',
        'usage_hold',
        'usage_hold_release'
    ));

-- Retained content volume (sum of stored chunk text bytes) — ADR §2.2 primary soft/hard.
INSERT INTO quota_limits (plan_id, metric_type, soft_limit, hard_limit) VALUES
    ('free', 'retained_content_bytes', 8589934592, 16106127360),
    ('plus', 'retained_content_bytes', 85899345920, 171798691840),
    ('pro', 'retained_content_bytes', 322122547200, 644245094400)
ON CONFLICT (plan_id, metric_type) DO UPDATE
SET soft_limit = EXCLUDED.soft_limit,
    hard_limit = EXCLUDED.hard_limit;
