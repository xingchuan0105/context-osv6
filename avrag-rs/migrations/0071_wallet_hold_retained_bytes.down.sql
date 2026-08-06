SELECT set_config('app.current_role', 'super_admin', true);

DELETE FROM quota_limits WHERE metric_type = 'retained_content_bytes';

ALTER TABLE wallet_ledger DROP CONSTRAINT IF EXISTS wallet_ledger_kind_check;
ALTER TABLE wallet_ledger ADD CONSTRAINT wallet_ledger_kind_check
    CHECK (kind IN ('signup_grant', 'referral_bonus', 'topup', 'usage_debit'));
