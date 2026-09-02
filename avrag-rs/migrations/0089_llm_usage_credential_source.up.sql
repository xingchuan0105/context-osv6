-- Chat-first W3: per-call credential attribution on usage segments.
-- `credential_source` = official | byok; feature/stage keep carrying the
-- component role, so every llm_usage_events row explains payer + credential.

ALTER TABLE llm_usage_events
    ADD COLUMN IF NOT EXISTS credential_source TEXT NOT NULL DEFAULT 'official';
