-- ADR-0010: private use is not sold as plan token walls.
-- Rolling 5h/7d limits were ADR-0006 residual product rights; 0/0 = unlimited
-- (same convention as usage_limit/service.rs and load_usage_window).
-- Protective spend gates remain wallet balance / BYOK, not free-plan unit caps.

UPDATE usage_limit_plan_policies
SET
    rolling_5h_limit_units = 0,
    rolling_7d_limit_units = 0,
    updated_at = now();

-- Drop any residual per-user overrides that re-enable the old token wall.
UPDATE usage_limit_user_overrides
SET
    rolling_5h_limit_units = 0,
    rolling_7d_limit_units = 0,
    enabled = false,
    updated_at = now()
WHERE enabled = true
   OR COALESCE(rolling_5h_limit_units, 0) > 0
   OR COALESCE(rolling_7d_limit_units, 0) > 0;
