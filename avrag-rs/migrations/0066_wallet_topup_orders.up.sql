-- ADR-0010 PR5: distinguish subscription vs wallet top-up on billing_orders.
-- Reuses existing Alipay F2F pending-order rail; pack_id is stored in plan_id
-- for product_kind = wallet_topup (amount_cents == amount_fen for CNY).

SELECT set_config('app.current_role', 'super_admin', true);

ALTER TABLE billing_orders
    ADD COLUMN IF NOT EXISTS product_kind TEXT NOT NULL DEFAULT 'subscription';

-- Normalize any unexpected values before the check constraint.
UPDATE billing_orders
SET product_kind = 'subscription'
WHERE product_kind IS NULL
   OR product_kind NOT IN ('subscription', 'wallet_topup');

ALTER TABLE billing_orders
    DROP CONSTRAINT IF EXISTS billing_orders_product_kind_check;

ALTER TABLE billing_orders
    ADD CONSTRAINT billing_orders_product_kind_check
    CHECK (product_kind IN ('subscription', 'wallet_topup'));

CREATE INDEX IF NOT EXISTS idx_billing_orders_product_kind
    ON billing_orders (product_kind)
    WHERE product_kind = 'wallet_topup';
