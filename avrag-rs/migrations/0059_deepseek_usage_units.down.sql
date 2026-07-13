-- Reverse 0059: drop new columns and restore pre-0059 rolling limits (0037 values).

UPDATE usage_limit_plan_policies SET
    rolling_5h_limit_units = 100000,
    rolling_7d_limit_units = 400000,
    updated_at = now()
WHERE plan_id = 'free';

UPDATE usage_limit_plan_policies SET
    rolling_5h_limit_units = 600000,
    rolling_7d_limit_units = 4000000,
    updated_at = now()
WHERE plan_id = 'plus';

UPDATE usage_limit_plan_policies SET
    rolling_5h_limit_units = 2500000,
    rolling_7d_limit_units = 15000000,
    updated_at = now()
WHERE plan_id = 'pro';

ALTER TABLE usage_limit_plan_policies
    DROP COLUMN IF EXISTS margin_multiplier;

ALTER TABLE llm_model_weights
    DROP COLUMN IF EXISTS cache_hit_unit_rate;

ALTER TABLE llm_usage_events
    DROP COLUMN IF EXISTS cached_tokens;
