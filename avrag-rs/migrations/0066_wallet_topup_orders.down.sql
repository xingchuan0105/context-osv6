-- Reverse ADR-0010 PR5 product_kind on billing_orders.
SELECT set_config('app.current_role', 'super_admin', true);

DROP INDEX IF EXISTS idx_billing_orders_product_kind;

ALTER TABLE billing_orders
    DROP CONSTRAINT IF EXISTS billing_orders_product_kind_check;

ALTER TABLE billing_orders
    DROP COLUMN IF EXISTS product_kind;
