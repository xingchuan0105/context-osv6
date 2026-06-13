-- 0041_legal_acceptances.up.sql
-- 记录用户对法律协议（ToS/Privacy）的同意历史。
-- 注册时必填；支付时可追加 billing_terms_version。

CREATE TABLE IF NOT EXISTS legal_acceptances (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    terms_version   TEXT NOT NULL,
    privacy_version TEXT NOT NULL,
    accepted_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    ip_address      TEXT,
    user_agent      TEXT,
    context         TEXT NOT NULL DEFAULT 'register'  -- 'register' | 'payment' | 're_acceptance'
);

CREATE INDEX idx_legal_acceptances_user_id ON legal_acceptances(user_id);
CREATE INDEX idx_legal_acceptances_accepted_at ON legal_acceptances(accepted_at);

COMMENT ON TABLE legal_acceptances IS '用户法律协议同意记录，用于合规审计';
COMMENT ON COLUMN legal_acceptances.context IS '同意场景：register=注册, payment=支付, re_acceptance=重新确认';
