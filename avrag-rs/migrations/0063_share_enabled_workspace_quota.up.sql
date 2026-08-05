-- ADR-0010 PR1: share_enabled workspace quota (max shared workspaces by plan).
-- Billing unit: workspaces.share_enabled = true. Does not change token rolling walls.

ALTER TABLE workspaces
    ADD COLUMN IF NOT EXISTS share_enabled BOOLEAN NOT NULL DEFAULT false;

-- Backfill: already externally shareable workspaces count toward quota.
UPDATE workspaces w
SET share_enabled = true
WHERE w.share_enabled = false
  AND (
    w.access_level IN ('link', 'public')
    OR EXISTS (
        SELECT 1
        FROM share_tokens st
        WHERE st.workspace_id = w.id
          AND st.revoked_at IS NULL
          AND (st.expires_at IS NULL OR st.expires_at > now())
    )
  );

CREATE INDEX IF NOT EXISTS idx_workspaces_owner_share_enabled
    ON workspaces (owner_id)
    WHERE share_enabled = true;

-- Plan policy: free=3, plus=10, pro=100 (default 3 for unknown/legacy rows).
ALTER TABLE usage_limit_plan_policies
    ADD COLUMN IF NOT EXISTS max_shared_workspaces INTEGER NOT NULL DEFAULT 3;

UPDATE usage_limit_plan_policies
SET max_shared_workspaces = 3, updated_at = now()
WHERE plan_id = 'free';

UPDATE usage_limit_plan_policies
SET max_shared_workspaces = 10, updated_at = now()
WHERE plan_id = 'plus';

UPDATE usage_limit_plan_policies
SET max_shared_workspaces = 100, updated_at = now()
WHERE plan_id = 'pro';
