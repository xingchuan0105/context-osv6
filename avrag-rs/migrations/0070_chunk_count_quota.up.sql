-- ADR-0010 §2.2: chunk_count hard/soft guardrails (retained index proxy).
-- Free: soft 300k / hard 800k; Plus: soft 3M / hard 6M; Pro: soft 15M / hard 30M.

INSERT INTO quota_limits (plan_id, metric_type, soft_limit, hard_limit) VALUES
    ('free', 'chunk_count', 300000, 800000),
    ('plus', 'chunk_count', 3000000, 6000000),
    ('pro', 'chunk_count', 15000000, 30000000)
ON CONFLICT (plan_id, metric_type) DO UPDATE
SET soft_limit = EXCLUDED.soft_limit,
    hard_limit = EXCLUDED.hard_limit;
