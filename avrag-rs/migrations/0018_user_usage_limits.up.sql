-- Phase 1: Shadow Mode — User-level LLM usage ledger
-- PRD: SINGLE_USER_USAGE_LIMIT_PRD.md

-- 1. LLM usage events (per-user, per-call granularity)
CREATE TABLE IF NOT EXISTS llm_usage_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    feature TEXT NOT NULL,        -- summary|planner|answer|search|chat
    stage TEXT NOT NULL DEFAULT 'unknown',
    provider TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL DEFAULT '',
    prompt_tokens BIGINT NOT NULL DEFAULT 0,
    completion_tokens BIGINT NOT NULL DEFAULT 0,
    total_tokens BIGINT NOT NULL DEFAULT 0,
    usage_units BIGINT NOT NULL DEFAULT 0,
    usage_source TEXT NOT NULL DEFAULT 'actual',  -- actual|estimated
    session_id UUID,
    document_id UUID,
    request_id TEXT,
    trace_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_llm_usage_user_time ON llm_usage_events (user_id, created_at DESC);
CREATE INDEX idx_llm_usage_user_feature ON llm_usage_events (user_id, feature, created_at DESC);
CREATE INDEX idx_llm_usage_org_user ON llm_usage_events (org_id, user_id, created_at DESC);

-- 2. Model weight table (token → usage_units conversion)
CREATE TABLE IF NOT EXISTS llm_model_weights (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    input_unit_rate DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    output_unit_rate DOUBLE PRECISION NOT NULL DEFAULT 2.0,
    enabled BOOLEAN NOT NULL DEFAULT true,
    effective_from TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(provider, model)
);

INSERT INTO llm_model_weights (provider, model, input_unit_rate, output_unit_rate) VALUES
    ('dashscope', 'qwen3.5-flash', 0.5, 1.0),
    ('dmxapi', 'gemini-3-flash-preview-thinking', 1.0, 3.0),
    ('dmxapi', 'gemini-3.1-flash-lite-preview', 0.5, 1.5),
    ('dashscope', 'qwen3.5-plus', 1.0, 2.0),
    ('siliconflow', 'Qwen/Qwen3-Embedding-8B', 0.1, 0.1),
    ('perplexity', 'nvidia/nemotron-3-super-120b-a12b', 1.5, 3.0)
ON CONFLICT (provider, model) DO NOTHING;

-- 3. Plan-level default limits
CREATE TABLE IF NOT EXISTS usage_limit_plan_policies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plan_id TEXT NOT NULL UNIQUE,
    rolling_5h_limit_units BIGINT NOT NULL DEFAULT 100,
    rolling_7d_limit_units BIGINT NOT NULL DEFAULT 1000,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO usage_limit_plan_policies (plan_id, rolling_5h_limit_units, rolling_7d_limit_units) VALUES
    ('free', 50, 500),
    ('pro', 200, 2000),
    ('enterprise', 0, 0)  -- 0 = unlimited
ON CONFLICT (plan_id) DO NOTHING;

-- 4. User-level overrides
CREATE TABLE IF NOT EXISTS usage_limit_user_overrides (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    rolling_5h_limit_units BIGINT,  -- NULL = use plan default
    rolling_7d_limit_units BIGINT,  -- NULL = use plan default
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
