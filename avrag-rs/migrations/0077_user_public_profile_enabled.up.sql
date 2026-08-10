-- Owner opt-in for the public sharer profile endpoint
-- (GET /api/public/users/{user_id}/shares). Default off.

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS public_profile_enabled BOOLEAN NOT NULL DEFAULT FALSE;

COMMENT ON COLUMN users.public_profile_enabled IS 'Opt-in: expose public sharer profile + active shares; default off';
