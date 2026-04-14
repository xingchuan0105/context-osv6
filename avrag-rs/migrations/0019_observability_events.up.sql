CREATE TABLE IF NOT EXISTS product_events (
    event_id UUID PRIMARY KEY,
    event_time TIMESTAMPTZ NOT NULL,
    event_date DATE NOT NULL,
    user_id UUID NOT NULL,
    session_id UUID,
    notebook_id UUID,
    surface TEXT NOT NULL,
    event_name TEXT NOT NULL,
    result TEXT NOT NULL,
    request_id TEXT,
    trace_id TEXT,
    client_platform TEXT NOT NULL DEFAULT 'web',
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE TABLE IF NOT EXISTS cost_events (
    event_id UUID PRIMARY KEY,
    event_time TIMESTAMPTZ NOT NULL,
    event_date DATE NOT NULL,
    user_id UUID NOT NULL,
    session_id UUID,
    notebook_id UUID,
    event_name TEXT NOT NULL,
    feature TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    prompt_tokens BIGINT NOT NULL DEFAULT 0,
    completion_tokens BIGINT NOT NULL DEFAULT 0,
    embedding_tokens BIGINT NOT NULL DEFAULT 0,
    usage_units BIGINT NOT NULL DEFAULT 0,
    storage_bytes_delta BIGINT NOT NULL DEFAULT 0,
    external_call_count BIGINT NOT NULL DEFAULT 0,
    source TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE TABLE IF NOT EXISTS daily_user_metrics (
    event_date DATE NOT NULL,
    user_id UUID NOT NULL,
    is_dau BOOLEAN NOT NULL DEFAULT false,
    is_new_user BOOLEAN NOT NULL DEFAULT false,
    is_activated BOOLEAN NOT NULL DEFAULT false,
    chat_count BIGINT NOT NULL DEFAULT 0,
    search_count BIGINT NOT NULL DEFAULT 0,
    upload_count BIGINT NOT NULL DEFAULT 0,
    shared_kb_open_count BIGINT NOT NULL DEFAULT 0,
    llm_prompt_tokens BIGINT NOT NULL DEFAULT 0,
    llm_completion_tokens BIGINT NOT NULL DEFAULT 0,
    embedding_tokens BIGINT NOT NULL DEFAULT 0,
    storage_bytes BIGINT NOT NULL DEFAULT 0,
    usage_units BIGINT NOT NULL DEFAULT 0,
    estimated_cost_cents BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (event_date, user_id)
);

CREATE TABLE IF NOT EXISTS daily_product_metrics (
    event_date DATE PRIMARY KEY,
    dau BIGINT NOT NULL DEFAULT 0,
    new_users BIGINT NOT NULL DEFAULT 0,
    activated_users BIGINT NOT NULL DEFAULT 0,
    daily_chat_users BIGINT NOT NULL DEFAULT 0,
    daily_search_users BIGINT NOT NULL DEFAULT 0,
    daily_upload_users BIGINT NOT NULL DEFAULT 0,
    daily_shared_kb_users BIGINT NOT NULL DEFAULT 0,
    total_llm_prompt_tokens BIGINT NOT NULL DEFAULT 0,
    total_llm_completion_tokens BIGINT NOT NULL DEFAULT 0,
    total_embedding_tokens BIGINT NOT NULL DEFAULT 0,
    total_upload_bytes BIGINT NOT NULL DEFAULT 0,
    total_estimated_cost_cents BIGINT NOT NULL DEFAULT 0,
    cost_per_dau_cents BIGINT NOT NULL DEFAULT 0,
    cost_per_activated_user_cents BIGINT NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS user_anomalies (
    anomaly_id UUID PRIMARY KEY,
    detected_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    user_id UUID NOT NULL,
    anomaly_kind TEXT NOT NULL,
    severity TEXT NOT NULL,
    signature TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE INDEX IF NOT EXISTS idx_product_events_event_date ON product_events(event_date);
CREATE INDEX IF NOT EXISTS idx_product_events_user_date ON product_events(user_id, event_date);
CREATE INDEX IF NOT EXISTS idx_product_events_name_date ON product_events(event_name, event_date);
CREATE INDEX IF NOT EXISTS idx_cost_events_event_date ON cost_events(event_date);
CREATE INDEX IF NOT EXISTS idx_cost_events_user_date ON cost_events(user_id, event_date);
CREATE INDEX IF NOT EXISTS idx_cost_events_feature_date ON cost_events(feature, event_date);
CREATE INDEX IF NOT EXISTS idx_user_anomalies_detected_at ON user_anomalies(detected_at DESC);
CREATE UNIQUE INDEX IF NOT EXISTS idx_user_anomalies_signature ON user_anomalies(signature);
