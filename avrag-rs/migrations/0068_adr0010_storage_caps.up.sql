-- ADR-0010 §2.2: private storage guardrails (retained md+index, not original files).
-- Free: soft ~8GB / hard ~15GB; Plus: soft 80GB / hard ~160GB; Pro: soft 300GB / hard ~600GB.
-- These are anti-abuse floors, not upsell SKUs.

INSERT INTO quota_limits (plan_id, metric_type, soft_limit, hard_limit) VALUES
    ('free', 'storage_bytes', 8589934592, 16106127360),
    ('plus', 'storage_bytes', 85899345920, 171798691840),
    ('pro', 'storage_bytes', 322122547200, 644245094400)
ON CONFLICT (plan_id, metric_type) DO UPDATE
SET soft_limit = EXCLUDED.soft_limit,
    hard_limit = EXCLUDED.hard_limit;
