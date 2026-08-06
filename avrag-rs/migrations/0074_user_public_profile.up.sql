-- W6 / ADR-0010 #8: public-facing owner card fields for share pages.
-- avatar/banner store object-store paths; served via public media route.

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS bio TEXT,
    ADD COLUMN IF NOT EXISTS contact_url TEXT,
    ADD COLUMN IF NOT EXISTS avatar_object_path TEXT,
    ADD COLUMN IF NOT EXISTS banner_object_path TEXT;

COMMENT ON COLUMN users.bio IS 'Public bio for share Owner card; optional';
COMMENT ON COLUMN users.contact_url IS 'Public contact/link URL for share Owner card; optional';
COMMENT ON COLUMN users.avatar_object_path IS 'Object-store path for avatar image; optional';
COMMENT ON COLUMN users.banner_object_path IS 'Object-store path for banner image; optional';
