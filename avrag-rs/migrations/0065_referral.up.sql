-- ADR-0010 PR4: referral codes + bilateral ¥5 (500 fen) grant.
-- Inviter quota: 5 + floor(lifetime_paid_topup_fen / 5000). Only status=rewarded counts.
-- Does not increase share workspace quota.

SELECT set_config('app.current_role', 'super_admin', true);

CREATE TABLE IF NOT EXISTS referral_codes (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    -- Stable short code, e.g. COS-A1B2C3 (unique, case-sensitive storage; lookup normalized).
    code TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at TIMESTAMPTZ NULL,
    CONSTRAINT referral_codes_code_key UNIQUE (code)
);

CREATE TABLE IF NOT EXISTS referrals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    inviter_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- One referral binding per invitee (at most one code per registration).
    invitee_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'rewarded', 'rejected')),
    rewarded_at TIMESTAMPTZ NULL,
    reject_reason TEXT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT referrals_invitee_id_key UNIQUE (invitee_id)
);

CREATE INDEX IF NOT EXISTS idx_referrals_inviter_status
    ON referrals (inviter_id, status);

CREATE INDEX IF NOT EXISTS idx_referral_codes_code
    ON referral_codes (code)
    WHERE revoked_at IS NULL;

ALTER TABLE referral_codes ENABLE ROW LEVEL SECURITY;
ALTER TABLE referral_codes FORCE ROW LEVEL SECURITY;
ALTER TABLE referrals ENABLE ROW LEVEL SECURITY;
ALTER TABLE referrals FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS user_isolation_referral_codes ON referral_codes;
CREATE POLICY user_isolation_referral_codes ON referral_codes
    USING (
        user_id = NULLIF(current_setting('app.current_user', true), '')::uuid
        OR current_setting('app.current_role', true) IN ('super_admin', 'admin', 'ops_admin', 'finance_admin')
    )
    WITH CHECK (
        user_id = NULLIF(current_setting('app.current_user', true), '')::uuid
        OR current_setting('app.current_role', true) IN ('super_admin', 'admin', 'ops_admin', 'finance_admin')
    );

-- Inviter can read their outbound referrals; invitee can read own binding.
DROP POLICY IF EXISTS user_isolation_referrals ON referrals;
CREATE POLICY user_isolation_referrals ON referrals
    USING (
        inviter_id = NULLIF(current_setting('app.current_user', true), '')::uuid
        OR invitee_id = NULLIF(current_setting('app.current_user', true), '')::uuid
        OR current_setting('app.current_role', true) IN ('super_admin', 'admin', 'ops_admin', 'finance_admin')
    )
    WITH CHECK (
        inviter_id = NULLIF(current_setting('app.current_user', true), '')::uuid
        OR invitee_id = NULLIF(current_setting('app.current_user', true), '')::uuid
        OR current_setting('app.current_role', true) IN ('super_admin', 'admin', 'ops_admin', 'finance_admin')
    );
