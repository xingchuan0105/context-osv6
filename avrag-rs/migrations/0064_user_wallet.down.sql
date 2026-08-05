-- Reverse ADR-0010 PR3 wallet tables.
SELECT set_config('app.current_role', 'super_admin', true);

DROP POLICY IF EXISTS user_isolation_wallet_ledger ON wallet_ledger;
DROP POLICY IF EXISTS user_isolation_wallets ON wallets;

DROP TABLE IF EXISTS wallet_ledger;
DROP TABLE IF EXISTS wallets;
