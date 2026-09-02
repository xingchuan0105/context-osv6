-- Chat-first W3: independent BYOK purpose for Quick Chat (design 2026-09-02 §8.2).
-- `quick_chat` never borrows generic `llm` secrets; every account keeps at most
-- one ACTIVE quick_chat config so resolution never depends on row order.

SELECT set_config('app.current_role', 'super_admin', true);

DO $mig0088_purpose_check$
DECLARE
    check_name TEXT;
BEGIN
    SELECT con.conname INTO check_name
    FROM pg_constraint con
    JOIN pg_attribute att
      ON att.attrelid = con.conrelid
     AND att.attname = 'purpose'
     AND con.conkey = ARRAY[att.attnum]
    WHERE con.contype = 'c'
      AND con.conrelid = 'user_provider_secrets'::regclass;
    IF check_name IS NOT NULL THEN
        EXECUTE format('ALTER TABLE user_provider_secrets DROP CONSTRAINT %I', check_name);
    END IF;
END
$mig0088_purpose_check$;

ALTER TABLE user_provider_secrets
    ADD CONSTRAINT user_provider_secrets_purpose_check
    CHECK (purpose IN ('llm', 'embedding', 'rerank', 'quick_chat'));

-- Single active Quick Chat config per account (workspace overrides excluded:
-- quick_chat is an account-level credential like the official route it replaces).
CREATE UNIQUE INDEX IF NOT EXISTS uq_user_provider_secrets_quick_chat_active
    ON user_provider_secrets (owner_user_id)
    WHERE revoked_at IS NULL AND purpose = 'quick_chat';
