CREATE TABLE IF NOT EXISTS feature_flag_change_requests (
    id TEXT PRIMARY KEY,
    flag_key TEXT NOT NULL REFERENCES feature_flags(key) ON DELETE CASCADE,
    current_enabled BOOLEAN NOT NULL,
    requested_enabled BOOLEAN NOT NULL,
    reason TEXT NOT NULL,
    status TEXT NOT NULL,
    requested_by UUID NOT NULL,
    reviewed_by UUID,
    review_note TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    reviewed_at TIMESTAMPTZ,
    executed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_feature_flag_change_requests_flag_status
    ON feature_flag_change_requests(flag_key, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_feature_flag_change_requests_status
    ON feature_flag_change_requests(status, created_at DESC);
