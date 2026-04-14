CREATE POLICY admin_access_api_keys ON api_keys
    USING (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'));
