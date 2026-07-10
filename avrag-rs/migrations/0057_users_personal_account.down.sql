-- Reverse: re-add nullable org_id (no organizations FK); drop blocked.
SELECT set_config('app.current_role', 'super_admin', true);
ALTER TABLE users DISABLE ROW LEVEL SECURITY;
ALTER TABLE users DROP COLUMN IF EXISTS blocked;
ALTER TABLE users ADD COLUMN IF NOT EXISTS org_id UUID;
ALTER TABLE users ENABLE ROW LEVEL SECURITY;
