CREATE POLICY admin_access_organizations ON organizations
    USING (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'));

CREATE POLICY admin_access_users ON users
    USING (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'));

CREATE POLICY admin_access_notebooks ON notebooks
    USING (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'));

CREATE POLICY admin_access_documents ON documents
    USING (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'));

CREATE POLICY admin_access_chunks ON chunks
    USING (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'));

CREATE POLICY admin_access_chat_sessions ON chat_sessions
    USING (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'));

CREATE POLICY admin_access_chat_messages ON chat_messages
    USING (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'));

CREATE POLICY admin_access_audit_log ON audit_log
    USING (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'));

CREATE POLICY admin_access_usage_events ON usage_events
    USING (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'));

CREATE POLICY admin_access_notebook_members ON notebook_members
    USING (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'));

CREATE POLICY admin_access_share_tokens ON share_tokens
    USING (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'));
