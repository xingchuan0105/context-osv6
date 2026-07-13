-- Restore pre-workspace spelling (will fail if notebooks no longer exist).
CREATE OR REPLACE FUNCTION delete_user_cascade(target_user_id UUID)
RETURNS BIGINT AS $$
DECLARE
    deleted_count BIGINT := 0;
BEGIN
    DELETE FROM product_events WHERE user_id = target_user_id;
    DELETE FROM cost_events      WHERE user_id = target_user_id;
    DELETE FROM daily_user_metrics WHERE user_id = target_user_id;
    DELETE FROM user_anomalies   WHERE user_id = target_user_id;

    DELETE FROM usage_export_jobs WHERE user_id = target_user_id;
    DELETE FROM llm_usage_events WHERE user_id = target_user_id;
    DELETE FROM usage_limit_user_overrides WHERE user_id = target_user_id;

    DELETE FROM api_keys WHERE created_by = target_user_id;
    DELETE FROM share_access_logs WHERE accessor_user_id = target_user_id;

    DELETE FROM chat_sessions WHERE user_id = target_user_id;

    UPDATE workspaces SET owner_id = NULL WHERE owner_id = target_user_id;
    UPDATE ingestion_tasks SET requested_by = NULL WHERE requested_by = target_user_id;
    UPDATE share_tokens SET created_by = NULL WHERE created_by = target_user_id;
    UPDATE document_cleanup_tasks SET requested_by = NULL WHERE requested_by = target_user_id;
    UPDATE workspace_members SET invited_by = NULL WHERE invited_by = target_user_id;

    DELETE FROM users WHERE id = target_user_id;
    GET DIAGNOSTICS deleted_count = ROW_COUNT;

    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;
