ALTER TABLE workspaces
    DROP CONSTRAINT IF EXISTS workspaces_share_member_question_limit_check;
ALTER TABLE workspaces
    DROP CONSTRAINT IF EXISTS workspaces_share_anon_question_limit_check;
ALTER TABLE workspaces
    DROP COLUMN IF EXISTS share_member_question_limit,
    DROP COLUMN IF EXISTS share_anon_question_limit;
