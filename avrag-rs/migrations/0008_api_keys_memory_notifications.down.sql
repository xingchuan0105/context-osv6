DROP INDEX IF EXISTS idx_notification_outbox_org_status;
DROP INDEX IF EXISTS idx_notifications_user_created;
DROP INDEX IF EXISTS idx_dialogue_states_org_session;
DROP INDEX IF EXISTS idx_user_profiles_org_user;
DROP INDEX IF EXISTS idx_api_keys_key_hash;
DROP INDEX IF EXISTS idx_api_keys_org_notebook;

DROP POLICY IF EXISTS tenant_isolation_notification_outbox ON notification_outbox;
DROP POLICY IF EXISTS tenant_isolation_notifications ON notifications;
DROP POLICY IF EXISTS tenant_isolation_dialogue_states ON dialogue_states;
DROP POLICY IF EXISTS tenant_isolation_user_profiles ON user_profiles;
DROP POLICY IF EXISTS tenant_isolation_api_keys ON api_keys;

DROP TABLE IF EXISTS notification_outbox;
DROP TABLE IF EXISTS notifications;
DROP TABLE IF EXISTS dialogue_states;
DROP TABLE IF EXISTS user_profiles;
DROP TABLE IF EXISTS api_keys;
