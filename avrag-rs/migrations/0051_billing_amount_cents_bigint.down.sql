-- Revert amount_cents back to INT (int4). Values exceeding int4 range will fail.
ALTER TABLE billing_orders ALTER COLUMN amount_cents TYPE INT USING amount_cents::int;
