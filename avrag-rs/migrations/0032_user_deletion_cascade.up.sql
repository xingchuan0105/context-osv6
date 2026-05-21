-- Data subject deletion (GDPR right to erasure).
-- Cascade-deletes all personal data associated with a user while preserving
-- org-level shared assets (notebooks, documents) that have other members.

CREATE OR REPLACE FUNCTION delete_user_cascade(target_user_id UUID)
RETURNS BIGINT AS $$
DECLARE
    deleted_count BIGINT := 0;
BEGIN
    -- 1. Observability / analytics events (no FK constraint → explicit delete)
    DELETE FROM product_events WHERE user_id = target_user_id;
    DELETE FROM cost_events      WHERE user_id = target_user_id;
    DELETE FROM daily_user_metrics WHERE user_id = target_user_id;
    DELETE FROM user_anomalies   WHERE user_id = target_user_id;

    -- 2. Personal data owned by the user
    DELETE FROM api_keys WHERE created_by = target_user_id;
    DELETE FROM share_access_logs WHERE accessor_user_id = target_user_id;

    -- 3. Chat sessions cascade to chat_messages and dialogue_states automatically
    DELETE FROM chat_sessions WHERE user_id = target_user_id;

    -- 4. Clear user references that use SET NULL so orphan records remain valid
    UPDATE organizations SET owner_id = NULL WHERE owner_id = target_user_id;
    UPDATE ingestion_tasks SET requested_by = NULL WHERE requested_by = target_user_id;
    UPDATE share_tokens SET created_by = NULL WHERE created_by = target_user_id;
    UPDATE document_soft_delete_cleanup SET requested_by = NULL WHERE requested_by = target_user_id;
    UPDATE notebook_members SET added_by = NULL WHERE added_by = target_user_id;
    UPDATE notebook_members SET invited_by = NULL WHERE invited_by = target_user_id;

    -- 5. Finally delete the user row.
    -- Tables with ON DELETE CASCADE (notebook_members, user_preferences,
    -- user_profiles, user_usage_limits, password_reset_tickets, notifications)
    -- are cleaned up automatically by PostgreSQL.
    DELETE FROM users WHERE id = target_user_id;
    GET DIAGNOSTICS deleted_count = ROW_COUNT;

    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;
