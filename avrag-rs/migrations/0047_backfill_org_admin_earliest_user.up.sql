-- Promote the earliest user in each org to org_admin when no admin exists yet.

UPDATE users u
SET role = 'org_admin'
FROM (
    SELECT DISTINCT ON (org_id) id
    FROM users
    ORDER BY org_id, created_at ASC, id ASC
) first_users
WHERE u.id = first_users.id
  AND u.role = 'user'
  AND NOT EXISTS (
    SELECT 1
    FROM users admins
    WHERE admins.org_id = u.org_id
      AND admins.role IN ('org_admin', 'super_admin')
  );
