-- ADR-0010: BYOK resolve audit (no secret material) + optional official price rows.
-- Price table is optional override; runtime also accepts PLATFORM_OFFICIAL_RATES_JSON.

CREATE TABLE IF NOT EXISTS provider_secret_audit (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_user_id UUID NOT NULL,
    secret_id UUID,
    purpose TEXT NOT NULL,
    provider TEXT NOT NULL,
    action TEXT NOT NULL,
    workspace_id UUID,
    request_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_provider_secret_audit_owner_created
    ON provider_secret_audit (owner_user_id, created_at DESC);

CREATE TABLE IF NOT EXISTS platform_official_rates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_pattern TEXT NOT NULL DEFAULT '',
    model_contains TEXT NOT NULL,
    input_fen_per_mtok DOUBLE PRECISION NOT NULL,
    cache_fen_per_mtok DOUBLE PRECISION NOT NULL DEFAULT 0,
    output_fen_per_mtok DOUBLE PRECISION NOT NULL DEFAULT 0,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (provider_pattern, model_contains)
);

COMMENT ON TABLE provider_secret_audit IS 'BYOK key use/rotate audit; never stores plaintext secrets';
COMMENT ON TABLE platform_official_rates IS 'Optional official fen/1M rates for platform-proxy whitelist';
