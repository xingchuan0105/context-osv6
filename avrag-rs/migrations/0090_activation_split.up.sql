ALTER TABLE daily_user_metrics
    DROP COLUMN IF EXISTS is_activated,
    ADD COLUMN IF NOT EXISTS is_first_answer BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS is_cited_activation BOOLEAN NOT NULL DEFAULT false;

ALTER TABLE daily_product_metrics
    DROP COLUMN IF EXISTS activated_users,
    DROP COLUMN IF EXISTS cost_per_activated_user_cents,
    ADD COLUMN IF NOT EXISTS first_answer_users BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS cited_activation_users BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS cost_per_cited_activation_cents BIGINT NOT NULL DEFAULT 0;
