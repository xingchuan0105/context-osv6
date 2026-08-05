-- ADR-0010 PR3: B2C user wallet + append-only ledger.
-- Amount unit: integer fen (分). 100 fen = ¥1; signup grant = 2000 fen = ¥20.
-- kinds reserved: signup_grant, referral_bonus (PR4), topup (PR5), usage_debit (PR6).

SELECT set_config('app.current_role', 'super_admin', true);

CREATE TABLE IF NOT EXISTS wallets (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    -- Spendable balance in fen (分). Credits increase; debits decrease. Never negative.
    balance_fen BIGINT NOT NULL DEFAULT 0 CHECK (balance_fen >= 0),
    -- Lifetime paid top-ups only (excludes gifts/referral). Used by referral quota formula.
    lifetime_paid_topup_fen BIGINT NOT NULL DEFAULT 0 CHECK (lifetime_paid_topup_fen >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS wallet_ledger (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('signup_grant', 'referral_bonus', 'topup', 'usage_debit')),
    -- Signed fen delta: credit > 0, debit < 0.
    amount_fen BIGINT NOT NULL CHECK (amount_fen <> 0),
    balance_after_fen BIGINT NOT NULL CHECK (balance_after_fen >= 0),
    -- Global unique key for at-most-once application (e.g. signup_grant:<user_id>).
    idempotency_key TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT wallet_ledger_idempotency_key_key UNIQUE (idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_wallet_ledger_user_created
    ON wallet_ledger (user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_wallet_ledger_user_kind
    ON wallet_ledger (user_id, kind);

ALTER TABLE wallets ENABLE ROW LEVEL SECURITY;
ALTER TABLE wallets FORCE ROW LEVEL SECURITY;
ALTER TABLE wallet_ledger ENABLE ROW LEVEL SECURITY;
ALTER TABLE wallet_ledger FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS user_isolation_wallets ON wallets;
CREATE POLICY user_isolation_wallets ON wallets
    USING (
        user_id = NULLIF(current_setting('app.current_user', true), '')::uuid
        OR current_setting('app.current_role', true) IN ('super_admin', 'admin', 'ops_admin', 'finance_admin')
    )
    WITH CHECK (
        user_id = NULLIF(current_setting('app.current_user', true), '')::uuid
        OR current_setting('app.current_role', true) IN ('super_admin', 'admin', 'ops_admin', 'finance_admin')
    );

DROP POLICY IF EXISTS user_isolation_wallet_ledger ON wallet_ledger;
CREATE POLICY user_isolation_wallet_ledger ON wallet_ledger
    USING (
        user_id = NULLIF(current_setting('app.current_user', true), '')::uuid
        OR current_setting('app.current_role', true) IN ('super_admin', 'admin', 'ops_admin', 'finance_admin')
    )
    WITH CHECK (
        user_id = NULLIF(current_setting('app.current_user', true), '')::uuid
        OR current_setting('app.current_role', true) IN ('super_admin', 'admin', 'ops_admin', 'finance_admin')
    );
