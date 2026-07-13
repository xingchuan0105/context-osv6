-- DeepSeek-style usage units: cache hit column, per-plan margin M, recalculated rolling limits.
-- Product "约 tokens" stays free 100k/400k, plus 600k/4M, pro 2.5M/15M (5h/7d).
-- limit_units = ceil(T_approx / 1000 * M); M free=2.0 plus=1.5 pro=1.3.

ALTER TABLE llm_usage_events
    ADD COLUMN IF NOT EXISTS cached_tokens BIGINT NOT NULL DEFAULT 0;

ALTER TABLE llm_model_weights
    ADD COLUMN IF NOT EXISTS cache_hit_unit_rate DOUBLE PRECISION NOT NULL DEFAULT 0.02;

ALTER TABLE usage_limit_plan_policies
    ADD COLUMN IF NOT EXISTS margin_multiplier DOUBLE PRECISION NOT NULL DEFAULT 2.0;

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

-- Ensure default M for any other plan rows.
UPDATE usage_limit_plan_policies
SET margin_multiplier = COALESCE(margin_multiplier, 2.0)
WHERE margin_multiplier IS NULL;

INSERT INTO llm_model_weights (provider, model, input_unit_rate, cache_hit_unit_rate, output_unit_rate)
VALUES
    ('deepseek', 'deepseek-v4-flash', 1.0, 0.02, 2.0),
    ('deepseek', 'deepseek-v4-pro', 1.0, 0.02, 2.0)
ON CONFLICT (provider, model) DO UPDATE
SET
    input_unit_rate = EXCLUDED.input_unit_rate,
    cache_hit_unit_rate = EXCLUDED.cache_hit_unit_rate,
    output_unit_rate = EXCLUDED.output_unit_rate,
    updated_at = now();
