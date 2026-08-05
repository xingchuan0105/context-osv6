-- Reverse ADR-0010 PR7 cloud BYOK provider secrets.
SELECT set_config('app.current_role', 'super_admin', true);

DROP POLICY IF EXISTS user_isolation_user_provider_secrets ON user_provider_secrets;
DROP TABLE IF EXISTS user_provider_secrets;
