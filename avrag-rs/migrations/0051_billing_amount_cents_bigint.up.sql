-- Widen billing_orders.amount_cents from INT (int4) to BIGINT (int8).
--
-- `decimal_price_to_cents` already returns i64; the previous INT column forced a
-- silent `as i32` cast that would truncate amounts over ~$21.4M. BIGINT makes
-- the storage type match the in-memory type and removes the truncation hazard.
ALTER TABLE billing_orders ALTER COLUMN amount_cents TYPE BIGINT USING amount_cents::bigint;
