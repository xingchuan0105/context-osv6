DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM pg_policies
        WHERE schemaname = current_schema()
          AND tablename = 'document_cleanup_tasks'
          AND policyname = 'tenant_isolation_document_cleanup_tasks'
    ) THEN
        DROP POLICY tenant_isolation_document_cleanup_tasks ON document_cleanup_tasks;
    END IF;

    CREATE POLICY tenant_isolation_document_cleanup_tasks ON document_cleanup_tasks
        USING (org_id = NULLIF(current_setting('app.current_org', true), '')::uuid)
        WITH CHECK (org_id = NULLIF(current_setting('app.current_org', true), '')::uuid);
END $$;
