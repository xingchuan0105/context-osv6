ALTER TABLE daily_user_metrics
    DROP COLUMN IF EXISTS is_first_answer,
    DROP COLUMN IF EXISTS is_cited_activation,
    ADD COLUMN IF NOT EXISTS is_activated BOOLEAN NOT NULL DEFAULT false;

ALTER TABLE daily_product_metrics
    DROP COLUMN IF EXISTS first_answer_users,
    DROP COLUMN IF EXISTS cited_activation_users,
    DROP COLUMN IF EXISTS cost_per_cited_activation_cents,
    ADD COLUMN IF NOT EXISTS activated_users BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS cost_per_activated_user_cents BIGINT NOT NULL DEFAULT 0;
