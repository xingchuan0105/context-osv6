DROP INDEX IF EXISTS idx_password_reset_tickets_expires_at;
DROP INDEX IF EXISTS idx_password_reset_tickets_email;

DROP POLICY IF EXISTS admin_update_password_reset_tickets ON password_reset_tickets;
DROP POLICY IF EXISTS admin_insert_password_reset_tickets ON password_reset_tickets;
DROP POLICY IF EXISTS admin_access_password_reset_tickets ON password_reset_tickets;
DROP POLICY IF EXISTS admin_update_users_auth ON users;
DROP POLICY IF EXISTS admin_insert_users_auth ON users;
DROP POLICY IF EXISTS admin_update_organizations_auth ON organizations;
DROP POLICY IF EXISTS admin_insert_organizations_auth ON organizations;

DROP TABLE IF EXISTS password_reset_tickets;

ALTER TABLE users
    DROP COLUMN IF EXISTS password_updated_at,
    DROP COLUMN IF EXISTS password_hash;
