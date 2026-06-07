-- Rollback 5h/7d policies to 0018 seed values (the 0036 state; 0035 renamed
-- enterprise -> plus in the subscriptions table but left usage_limit_plan_policies
-- unchanged).
-- NOTE: rolling_5h_limit_units / rolling_7d_limit_units are NOT NULL columns
-- (see 0018), so we cannot blank them out -- we restore the pre-0037 numbers.
-- 'plus' is dropped because 0018 did not seed it; 0035's enterprise row stays.
DELETE FROM usage_limit_plan_policies WHERE plan_id = 'plus';

UPDATE usage_limit_plan_policies
SET rolling_5h_limit_units = 50,
    rolling_7d_limit_units = 500
WHERE plan_id = 'free';

UPDATE usage_limit_plan_policies
SET rolling_5h_limit_units = 200,
    rolling_7d_limit_units = 2000
WHERE plan_id = 'pro';

-- quota_limits capacity values were not substantively changed (Free matches
-- 0007 seed, Plus matches 0035 seed, Pro stays NULL from 0035), so no
-- rollback is needed for that table.
