-- ADR-0010 §4: per-share visitor question caps (Owner-configurable).
-- anon: default 10/day per visitor identity; 0 = no daily cap (platform RPM still applies).
-- member (registered/invite): NULL = unlimited; positive = per user_id / day.

ALTER TABLE workspaces
    ADD COLUMN IF NOT EXISTS share_anon_question_limit INT NOT NULL DEFAULT 10,
    ADD COLUMN IF NOT EXISTS share_member_question_limit INT NULL;

ALTER TABLE workspaces
    DROP CONSTRAINT IF EXISTS workspaces_share_anon_question_limit_check;
ALTER TABLE workspaces
    ADD CONSTRAINT workspaces_share_anon_question_limit_check
    CHECK (share_anon_question_limit >= 0);

ALTER TABLE workspaces
    DROP CONSTRAINT IF EXISTS workspaces_share_member_question_limit_check;
ALTER TABLE workspaces
    ADD CONSTRAINT workspaces_share_member_question_limit_check
    CHECK (share_member_question_limit IS NULL OR share_member_question_limit > 0);

COMMENT ON COLUMN workspaces.share_anon_question_limit IS
    'ADR-0010: daily question cap per anonymous visitor (edge_ip) on this share; 0 = unlimited';
COMMENT ON COLUMN workspaces.share_member_question_limit IS
    'ADR-0010: daily question cap per registered visitor user_id; NULL = unlimited';
