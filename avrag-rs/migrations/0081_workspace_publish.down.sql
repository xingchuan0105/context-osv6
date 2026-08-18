SELECT set_config('app.current_role', 'super_admin', true);

DROP POLICY IF EXISTS user_isolation_workspace_publish ON workspace_publish;
DROP TABLE IF EXISTS workspace_publish;
