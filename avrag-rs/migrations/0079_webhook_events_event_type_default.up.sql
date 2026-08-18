-- claim_webhook_with_lease historically omitted event_type; the column is NOT NULL
-- (0007_billing), so every provider notify failed at lease insert with 500 and
-- never marked billing_orders paid. Default unblocks any residual insert path;
-- application code must still send event_type explicitly.
ALTER TABLE webhook_events
    ALTER COLUMN event_type SET DEFAULT 'provider.delivery';
