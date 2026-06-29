-- Revert sole-user org_admin backfill to 'user'.

UPDATE users u
SET role = 'user'
WHERE u.role = 'org_admin'
  AND (
    SELECT COUNT(*)
    FROM users u2
    WHERE u2.org_id = u.org_id
  ) = 1;
