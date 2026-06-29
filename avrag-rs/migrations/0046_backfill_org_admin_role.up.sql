-- Promote legacy sole org creators from role 'user' to 'org_admin'.

UPDATE users u
SET role = 'org_admin'
WHERE u.role = 'user'
  AND (
    SELECT COUNT(*)
    FROM users u2
    WHERE u2.org_id = u.org_id
  ) = 1;
