-- ADR 0006: distinguish customer-billable LLM rows from internal (worker) metering.
ALTER TABLE llm_usage_events
  ADD COLUMN IF NOT EXISTS billable BOOLEAN NOT NULL DEFAULT true;

CREATE INDEX IF NOT EXISTS idx_llm_usage_user_billable_time
  ON llm_usage_events (user_id, billable, created_at DESC);
