ALTER TABLE user_profiles
    ADD COLUMN IF NOT EXISTS structured_profile JSONB NOT NULL DEFAULT '{}'::jsonb;
