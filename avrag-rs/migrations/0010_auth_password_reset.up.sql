ALTER TABLE users
    ADD COLUMN IF NOT EXISTS password_hash TEXT,
    ADD COLUMN IF NOT EXISTS password_updated_at TIMESTAMPTZ;

CREATE TABLE IF NOT EXISTS password_reset_tickets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    purpose TEXT NOT NULL DEFAULT 'password_reset',
    ticket_hash TEXT NOT NULL UNIQUE,
    code_hash TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    code_expires_at TIMESTAMPTZ,
    attempts INTEGER NOT NULL DEFAULT 0,
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE password_reset_tickets ENABLE ROW LEVEL SECURITY;
ALTER TABLE password_reset_tickets FORCE ROW LEVEL SECURITY;

CREATE POLICY admin_insert_organizations_auth ON organizations
    FOR INSERT
    WITH CHECK (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'));

CREATE POLICY admin_update_organizations_auth ON organizations
    FOR UPDATE
    USING (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'))
    WITH CHECK (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'));

CREATE POLICY admin_insert_users_auth ON users
    FOR INSERT
    WITH CHECK (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'));

CREATE POLICY admin_update_users_auth ON users
    FOR UPDATE
    USING (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'))
    WITH CHECK (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'));

CREATE POLICY admin_access_password_reset_tickets ON password_reset_tickets
    USING (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'));

CREATE POLICY admin_insert_password_reset_tickets ON password_reset_tickets
    FOR INSERT
    WITH CHECK (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'));

CREATE POLICY admin_update_password_reset_tickets ON password_reset_tickets
    FOR UPDATE
    USING (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'))
    WITH CHECK (nullif(current_setting('app.current_role', true), '') in ('super_admin', 'ops_admin', 'finance_admin'));

CREATE INDEX IF NOT EXISTS idx_password_reset_tickets_email ON password_reset_tickets(lower(email), purpose, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_password_reset_tickets_expires_at ON password_reset_tickets(expires_at);
