-- Revert to pre-ADR-0010 DeepSeek-style unit caps (migration 0059 values).
-- Not a product path — only for local rollback experiments.

UPDATE usage_limit_plan_policies SET
    margin_multiplier = 2.0,
    rolling_5h_limit_units = 200,
    rolling_7d_limit_units = 800,
    updated_at = now()
WHERE plan_id = 'free';

UPDATE usage_limit_plan_policies SET
    margin_multiplier = 1.5,
    rolling_5h_limit_units = 900,
    rolling_7d_limit_units = 6000,
    updated_at = now()
WHERE plan_id = 'plus';

UPDATE usage_limit_plan_policies SET
    margin_multiplier = 1.3,
    rolling_5h_limit_units = 3250,
    rolling_7d_limit_units = 19500,
    updated_at = now()
WHERE plan_id = 'pro';

UPDATE usage_limit_plan_policies SET
    rolling_5h_limit_units = 0,
    rolling_7d_limit_units = 0,
    updated_at = now()
WHERE plan_id = 'enterprise';
