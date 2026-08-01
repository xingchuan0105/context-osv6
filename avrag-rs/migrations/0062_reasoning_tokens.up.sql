-- Reasoning/thinking token accounting (Responses API split).
-- DeepSeek v4 reports reasoning tokens in output_tokens_details.reasoning_tokens;
-- persisted so reasoning-cost reporting and billing can split them out.

ALTER TABLE llm_usage_events
    ADD COLUMN IF NOT EXISTS reasoning_tokens BIGINT NOT NULL DEFAULT 0;
