-- Reverse ADR-0010 PR4 referral tables.
SELECT set_config('app.current_role', 'super_admin', true);

DROP POLICY IF EXISTS user_isolation_referrals ON referrals;
DROP POLICY IF EXISTS user_isolation_referral_codes ON referral_codes;

DROP TABLE IF EXISTS referrals;
DROP TABLE IF EXISTS referral_codes;
