-- Reverse 0080 desktop relay tokens.
SELECT set_config('app.current_role', 'super_admin', true);

DROP POLICY IF EXISTS user_isolation_desktop_tokens ON desktop_tokens;
DROP TABLE IF EXISTS desktop_tokens;
