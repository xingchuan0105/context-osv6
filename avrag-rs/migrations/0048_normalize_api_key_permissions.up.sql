-- Normalize workspace API key permissions to index/query only.

UPDATE api_keys
SET permissions = COALESCE(
        (
            SELECT array_agg(DISTINCT perm ORDER BY perm)
            FROM unnest(permissions) AS perm
            WHERE perm IN ('index', 'query')
        ),
        ARRAY['index', 'query']::text[]
    ),
    updated_at = NOW()
WHERE notebook_id IS NOT NULL;

UPDATE api_keys
SET permissions = ARRAY['index', 'query']::text[],
    updated_at = NOW()
WHERE notebook_id IS NOT NULL
  AND (
    permissions IS NULL
    OR cardinality(permissions) = 0
  );

-- Normalize org API key permissions to workspace.create/workspace.list only.

UPDATE api_keys
SET permissions = COALESCE(
        (
            SELECT array_agg(DISTINCT perm ORDER BY perm)
            FROM unnest(permissions) AS perm
            WHERE perm IN ('workspace.create', 'workspace.list')
        ),
        ARRAY['workspace.create', 'workspace.list']::text[]
    ),
    updated_at = NOW()
WHERE notebook_id IS NULL;

UPDATE api_keys
SET permissions = ARRAY['workspace.create', 'workspace.list']::text[],
    updated_at = NOW()
WHERE notebook_id IS NULL
  AND (
    permissions IS NULL
    OR cardinality(permissions) = 0
  );
