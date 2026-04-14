DROP POLICY IF EXISTS tenant_isolation_usage_events ON usage_events;
DROP POLICY IF EXISTS tenant_isolation_audit_log ON audit_log;
DROP POLICY IF EXISTS tenant_isolation_chat_messages ON chat_messages;
DROP POLICY IF EXISTS tenant_isolation_chat_sessions ON chat_sessions;
DROP POLICY IF EXISTS tenant_isolation_chunks ON chunks;
DROP POLICY IF EXISTS tenant_isolation_documents ON documents;
DROP POLICY IF EXISTS tenant_isolation_notebooks ON notebooks;
DROP POLICY IF EXISTS tenant_isolation_users ON users;

DROP TABLE IF EXISTS usage_events;
DROP TABLE IF EXISTS audit_log;
DROP TABLE IF EXISTS chat_messages;
DROP TABLE IF EXISTS chat_sessions;
DROP TABLE IF EXISTS chunks;
DROP TABLE IF EXISTS documents;
DROP TABLE IF EXISTS notebooks;
DROP TABLE IF EXISTS users;
DROP TABLE IF EXISTS organizations;
