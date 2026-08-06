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
