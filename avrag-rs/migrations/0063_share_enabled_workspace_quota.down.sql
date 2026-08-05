-- Reverse ADR-0010 PR1 share_enabled workspace quota.

DROP INDEX IF EXISTS idx_workspaces_owner_share_enabled;

ALTER TABLE workspaces
    DROP COLUMN IF EXISTS share_enabled;

ALTER TABLE usage_limit_plan_policies
    DROP COLUMN IF EXISTS max_shared_workspaces;
