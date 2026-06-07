-- 1) Refresh quota_limits: capacity values (pages / embedding / storage / llm_in / llm_out)
-- Capacity tiers unchanged; idempotent refresh only.
-- Note: Pro tier is intentionally absent here -- 0035 left Pro with NULL (unlimited) capacity.
INSERT INTO quota_limits (plan_id, metric_type, soft_limit, hard_limit) VALUES
    ('free', 'pages_processed', 100, 500),
    ('free', 'embedding_tokens', 100000, 500000),
    ('free', 'llm_input_tokens', 50000, 100000),
    ('free', 'llm_output_tokens', 25000, 50000),
    ('free', 'storage_bytes', 1073741824, 5368709120),
    ('plus', 'pages_processed', 5000, 10000),
    ('plus', 'embedding_tokens', 5000000, 10000000),
    ('plus', 'llm_input_tokens', 500000, 1000000),
    ('plus', 'llm_output_tokens', 250000, 500000),
    ('plus', 'storage_bytes', 5368709120, 10737418240)
ON CONFLICT (plan_id, metric_type) DO UPDATE
SET soft_limit = EXCLUDED.soft_limit, hard_limit = EXCLUDED.hard_limit;

-- 2) Refresh 5h/7d rolling limit policies (core change for pricing revamp).
-- 0018 seeded: free=50/500, pro=200/2000, enterprise=0/0 (unlimited). 0035 renamed
-- enterprise -> plus but did NOT update this table. This migration brings the
-- three revamped tiers (free / plus / pro) in line with spec §2.1 and leaves
-- the legacy enterprise row (0/0) untouched.
INSERT INTO usage_limit_plan_policies (plan_id, rolling_5h_limit_units, rolling_7d_limit_units) VALUES
    ('free',  100000,    400000),
    ('plus',  600000,    4000000),
    ('pro',   2500000,   15000000)
ON CONFLICT (plan_id) DO UPDATE
SET rolling_5h_limit_units = EXCLUDED.rolling_5h_limit_units,
    rolling_7d_limit_units = EXCLUDED.rolling_7d_limit_units;
