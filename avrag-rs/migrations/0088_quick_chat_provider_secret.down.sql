SELECT set_config('app.current_role', 'super_admin', true);

DROP INDEX IF EXISTS uq_user_provider_secrets_quick_chat_active;

ALTER TABLE user_provider_secrets DROP CONSTRAINT IF EXISTS user_provider_secrets_purpose_check;

-- Restore the 0067 check verbatim (rows with purpose='quick_chat' must be gone).
DELETE FROM user_provider_secrets WHERE purpose = 'quick_chat';

ALTER TABLE user_provider_secrets
    ADD CONSTRAINT user_provider_secrets_purpose_check
    CHECK (purpose IN ('llm', 'embedding', 'rerank'));
