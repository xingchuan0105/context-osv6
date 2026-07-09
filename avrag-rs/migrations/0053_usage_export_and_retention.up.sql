-- ADR 0006 §10: usage export jobs + retention index for 365d cleanup.
CREATE TABLE IF NOT EXISTS usage_export_jobs (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id UUID NOT NULL,
  user_id UUID NOT NULL,
  range_from TIMESTAMPTZ NOT NULL,
  range_to TIMESTAMPTZ NOT NULL,
  format TEXT NOT NULL DEFAULT 'csv',
  status TEXT NOT NULL DEFAULT 'pending',
  row_count INTEGER,
  result_text TEXT,
  error_message TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  completed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_usage_export_jobs_user_created
  ON usage_export_jobs (user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_usage_export_jobs_pending
  ON usage_export_jobs (status, created_at ASC)
  WHERE status = 'pending';

-- Speed 365-day retention deletes on llm_usage_events.
CREATE INDEX IF NOT EXISTS idx_llm_usage_created_at
  ON llm_usage_events (created_at);
