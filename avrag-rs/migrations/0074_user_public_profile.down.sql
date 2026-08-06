ALTER TABLE users
    DROP COLUMN IF EXISTS bio,
    DROP COLUMN IF EXISTS contact_url,
    DROP COLUMN IF EXISTS avatar_object_path,
    DROP COLUMN IF EXISTS banner_object_path;
