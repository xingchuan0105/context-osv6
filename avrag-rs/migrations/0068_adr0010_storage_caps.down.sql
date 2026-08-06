-- Restore pre-ADR-0010 storage caps (0037 / free-tier defaults).
INSERT INTO quota_limits (plan_id, metric_type, soft_limit, hard_limit) VALUES
    ('free', 'storage_bytes', 1073741824, 5368709120),
    ('plus', 'storage_bytes', 5368709120, 10737418240),
    ('pro', 'storage_bytes', 5368709120, 10737418240)
ON CONFLICT (plan_id, metric_type) DO UPDATE
SET soft_limit = EXCLUDED.soft_limit,
    hard_limit = EXCLUDED.hard_limit;
