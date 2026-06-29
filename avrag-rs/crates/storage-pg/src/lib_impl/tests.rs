#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn ingestion_retry_backoff_is_exponential_and_capped() {
        assert_eq!(ingestion_retry_backoff_seconds(0), 30);
        assert_eq!(ingestion_retry_backoff_seconds(1), 30);
        assert_eq!(ingestion_retry_backoff_seconds(2), 60);
        assert_eq!(ingestion_retry_backoff_seconds(3), 120);
        assert_eq!(ingestion_retry_backoff_seconds(9), 3600);
    }

    async fn insert_test_document_block(
        repo: &PgAppRepository,
        org_id: Uuid,
        notebook_id: Uuid,
        document_id: Uuid,
        block_id: &str,
    ) {
        let mut tx = repo.raw().begin().await.unwrap();
        sqlx::query("select set_config('app.current_org', $1, true)")
            .bind(org_id.to_string())
            .execute(tx.as_mut())
            .await
            .unwrap();
        sqlx::query(
            r#"
            insert into document_blocks (
                org_id, notebook_id, document_id, block_id, page, block_type, modality,
                text, parser_backend
            ) values ($1, $2, $3, $4, 1, 'paragraph', 'text', 'block text', 'test')
            "#,
        )
        .bind(org_id)
        .bind(notebook_id)
        .bind(document_id)
        .bind(block_id)
        .execute(tx.as_mut())
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    async fn count_document_blocks_for_org(
        repo: &PgAppRepository,
        org_id: Uuid,
        document_id: Uuid,
    ) -> i64 {
        let mut tx = repo.raw().begin().await.unwrap();
        sqlx::query("select set_config('app.current_org', $1, true)")
            .bind(org_id.to_string())
            .execute(tx.as_mut())
            .await
            .unwrap();
        let row = sqlx::query(
            "select count(*)::bigint as c from document_blocks where org_id = $1 and document_id = $2",
        )
        .bind(org_id)
        .bind(document_id)
        .fetch_one(tx.as_mut())
        .await
        .unwrap();
        tx.commit().await.unwrap();
        row.try_get("c").unwrap()
    }

    #[tokio::test]
    async fn document_ir_projection_deletes_are_tenant_scoped_when_database_available() {
        let Some(database_url) = env::var("DATABASE_URL").ok() else {
            return;
        };
        let repo = PgAppRepository::connect(&database_url).await.unwrap();
        repo.migrate().await.unwrap();

        let org_id = OrgId::from(Uuid::new_v4());
        let owner_org_uuid = org_id.into_uuid();
        let other_org_uuid = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let ctx = AuthContext::new(org_id, avrag_auth::SubjectKind::User)
            .with_actor_id(ActorId::new(user_id));
        let notebook = repo
            .create_notebook(&ctx, "ir tenant scope notebook", "ir tenant scope")
            .await
            .unwrap();
        let notebook_id = Uuid::parse_str(&notebook.id).unwrap();
        let document = repo
            .create_document(&ctx, notebook_id, "ir-tenant-scope.txt", 42, "text/plain")
            .await
            .unwrap();
        let document_id = Uuid::parse_str(&document.id).unwrap();
        let other_notebook_id = Uuid::new_v4();

        insert_test_document_block(
            &repo,
            owner_org_uuid,
            notebook_id,
            document_id,
            "owner-clear-block",
        )
        .await;
        insert_test_document_block(
            &repo,
            other_org_uuid,
            other_notebook_id,
            document_id,
            "other-clear-block",
        )
        .await;

        repo.clear_document_ir_projection(&ctx, document_id)
            .await
            .unwrap();

        assert_eq!(
            count_document_blocks_for_org(&repo, owner_org_uuid, document_id).await,
            0
        );
        assert_eq!(
            count_document_blocks_for_org(&repo, other_org_uuid, document_id).await,
            1,
            "clear_document_ir_projection must not delete another tenant's derived rows"
        );

        insert_test_document_block(
            &repo,
            owner_org_uuid,
            notebook_id,
            document_id,
            "owner-replace-block",
        )
        .await;
        let replacement = StoredDocumentBlock {
            block_id: "owner-replacement-block".to_string(),
            parse_run_id: None,
            page: Some(1),
            block_type: "paragraph".to_string(),
            modality: "text".to_string(),
            text: "replacement text".to_string(),
            summary_text: None,
            caption: None,
            asset_refs: serde_json::json!([]),
            section_path: serde_json::json!([]),
            source_locator_json: serde_json::json!({}),
            parser_backend: "test".to_string(),
            metadata_json: serde_json::json!({}),
        };

        repo.replace_document_blocks(&ctx, notebook_id, document_id, &[replacement])
            .await
            .unwrap();

        assert_eq!(
            count_document_blocks_for_org(&repo, owner_org_uuid, document_id).await,
            1
        );
        assert_eq!(
            count_document_blocks_for_org(&repo, other_org_uuid, document_id).await,
            1,
            "replace_document_blocks must not delete another tenant's derived rows"
        );
    }

    #[test]
    fn derived_document_tables_have_tenant_rls_migration() {
        let migration = include_str!("../../../../migrations/0029_document_derived_rls.up.sql");

        for table in [
            "document_assets",
            "document_multimodal_chunks",
            "document_parse_runs",
            "document_blocks",
        ] {
            assert!(
                migration.contains(&format!("ALTER TABLE {table} ENABLE ROW LEVEL SECURITY")),
                "{table} should enable row-level security"
            );
            assert!(
                migration.contains(&format!("ALTER TABLE {table} FORCE ROW LEVEL SECURITY")),
                "{table} should force row-level security"
            );
            assert!(
                migration.contains(&format!("CREATE POLICY tenant_isolation_{table} ON {table}")),
                "{table} should have tenant isolation policy"
            );
        }
    }

    #[tokio::test]
    async fn renew_ingestion_task_lock_matches_processing_task_lease_when_database_available() {
        let Some(database_url) = env::var("DATABASE_URL").ok() else {
            return;
        };
        let repo = PgAppRepository::connect(&database_url).await.unwrap();
        repo.migrate().await.unwrap();

        let org_id = OrgId::from(Uuid::new_v4());
        let user_id = Uuid::new_v4();
        let ctx = AuthContext::new(org_id, avrag_auth::SubjectKind::User)
            .with_actor_id(ActorId::new(user_id));

        let notebook = repo
            .create_notebook(&ctx, "lease renewal test notebook", "lease renewal test")
            .await
            .unwrap();
        let document = repo
            .create_document(
                &ctx,
                Uuid::parse_str(&notebook.id).unwrap(),
                "lease-renewal-test.txt",
                42,
                "text/plain",
            )
            .await
            .unwrap();

        let task = ingestion::build_ingest_task(
            org_id.to_string(),
            notebook.id.clone(),
            document.id.clone(),
            Some(user_id.to_string()),
            ingestion::IngestDocumentPayload {
                source_uri: "s3://bucket/org/notebook/doc/lease-renewal-test.txt".to_string(),
                object_path: "org/notebook/doc/lease-renewal-test.txt".to_string(),
                mime_type: "text/plain".to_string(),
                filename: "lease-renewal-test.txt".to_string(),
                file_size: 42,
            },
        );
        assert!(repo.enqueue_ingestion_task(&task).await.unwrap());
        let task_id = Uuid::parse_str(&task.task_id).unwrap();
        let lock_token = Uuid::new_v4();
        sqlx::query(
            r#"
            update ingestion_tasks
            set status = 'processing',
                locked_at = now() - interval '5 minutes',
                locked_by = 'lease-renewal-test-worker',
                lock_token = $2,
                attempt_count = attempt_count + 1,
                updated_at = now()
            where task_id = $1
            "#,
        )
        .bind(task_id)
        .bind(lock_token)
        .execute(repo.raw())
        .await
        .unwrap();

        let lock_token = lock_token.to_string();
        assert!(
            repo.renew_ingestion_task_lock(&task.task_id, &lock_token)
                .await
                .unwrap()
        );
        assert!(
            !repo
                .renew_ingestion_task_lock(&task.task_id, &Uuid::new_v4().to_string())
                .await
                .unwrap()
        );
        assert_eq!(
            repo.complete_ingestion_task(&task.task_id, Some(&lock_token))
                .await
                .unwrap(),
            TaskCompletionOutcome::Completed
        );
    }

    #[tokio::test]
    async fn delete_document_soft_deletes_and_enqueues_cleanup_once_when_database_available() {
        let Some(database_url) = env::var("DATABASE_URL").ok() else {
            return;
        };
        let repo = PgAppRepository::connect(&database_url).await.unwrap();
        repo.migrate().await.unwrap();

        let org_id = OrgId::from(Uuid::new_v4());
        let user_id = Uuid::new_v4();
        let ctx = AuthContext::new(org_id, avrag_auth::SubjectKind::User)
            .with_actor_id(ActorId::new(user_id));

        let notebook = repo
            .create_notebook(&ctx, "soft delete test notebook", "soft delete test")
            .await
            .unwrap();
        let notebook_id = Uuid::parse_str(&notebook.id).unwrap();
        let document = repo
            .create_document(&ctx, notebook_id, "delete-me.txt", 42, "text/plain")
            .await
            .unwrap();
        let document_id = Uuid::parse_str(&document.id).unwrap();
        let task = ingestion::build_ingest_task(
            org_id.to_string(),
            notebook.id.clone(),
            document.id.clone(),
            Some(user_id.to_string()),
            ingestion::IngestDocumentPayload {
                source_uri: "s3://bucket/org/notebook/doc/delete-me.txt".to_string(),
                object_path: "org/notebook/doc/delete-me.txt".to_string(),
                mime_type: "text/plain".to_string(),
                filename: "delete-me.txt".to_string(),
                file_size: 42,
            },
        );
        assert!(repo.enqueue_ingestion_task(&task).await.unwrap());

        assert_eq!(
            repo.delete_document(&ctx, document_id).await.unwrap(),
            DocumentDeletionOutcome::Queued {
                task_inserted: true
            }
        );
        assert_eq!(
            repo.delete_document(&ctx, document_id).await.unwrap(),
            DocumentDeletionOutcome::AlreadyDeleting {
                task_inserted: false
            }
        );
        assert_eq!(
            repo.get_document_status(&ctx, document_id).await.unwrap(),
            Some(DocumentStatus::Deleting)
        );
        assert_eq!(
            repo.count_document_cleanup_tasks_for_document(&ctx, document_id)
                .await
                .unwrap(),
            1
        );
        let task_row = sqlx::query("select status from ingestion_tasks where task_id = $1")
            .bind(Uuid::parse_str(&task.task_id).unwrap())
            .fetch_one(repo.raw())
            .await
            .unwrap();
        assert_eq!(
            task_row.try_get::<String, _>("status").unwrap(),
            "dead_letter"
        );
        assert!(
            repo.list_documents(&ctx, Some(notebook_id), Some(document_id))
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn document_cleanup_task_claim_fail_complete_and_db_cleanup_when_database_available() {
        let Some(database_url) = env::var("DATABASE_URL").ok() else {
            return;
        };
        let repo = PgAppRepository::connect(&database_url).await.unwrap();
        repo.migrate().await.unwrap();

        let org_id = OrgId::from(Uuid::new_v4());
        let user_id = Uuid::new_v4();
        let ctx = AuthContext::new(org_id, avrag_auth::SubjectKind::User)
            .with_actor_id(ActorId::new(user_id));
        let other_org_id = OrgId::from(Uuid::new_v4());
        let other_user_id = Uuid::new_v4();
        let other_ctx = AuthContext::new(other_org_id, avrag_auth::SubjectKind::User)
            .with_actor_id(ActorId::new(other_user_id));
        let other_notebook = repo
            .create_notebook(
                &other_ctx,
                "cleanup other tenant notebook",
                "cleanup other tenant",
            )
            .await
            .unwrap();
        let other_notebook_id = Uuid::parse_str(&other_notebook.id).unwrap();
        let notebook = repo
            .create_notebook(&ctx, "cleanup task test notebook", "cleanup task test")
            .await
            .unwrap();
        let notebook_id = Uuid::parse_str(&notebook.id).unwrap();
        let document = repo
            .create_document(&ctx, notebook_id, "cleanup-me.txt", 42, "text/plain")
            .await
            .unwrap();
        let document_id = Uuid::parse_str(&document.id).unwrap();
        assert_eq!(
            repo.delete_document(&ctx, document_id).await.unwrap(),
            DocumentDeletionOutcome::Queued {
                task_inserted: true
            }
        );

        let claimed = repo
            .claim_next_document_cleanup_task("cleanup-test-worker", Some(60))
            .await
            .unwrap()
            .expect("cleanup task should be claimed");
        assert_eq!(claimed.org_id, org_id.into_uuid());
        assert_eq!(claimed.notebook_id, notebook_id);
        assert_eq!(claimed.document_id, document_id);
        let lock_token = claimed.lock_token.expect("claim must return lock token");
        assert!(
            repo.renew_document_cleanup_task_lock(claimed.task_id, lock_token)
                .await
                .unwrap()
        );
        assert!(
            repo.document_cleanup_task_lease_is_current(claimed.task_id, lock_token)
                .await
                .unwrap()
        );
        assert!(
            !repo
                .document_cleanup_task_lease_is_current(claimed.task_id, Uuid::new_v4())
                .await
                .unwrap()
        );
        assert!(
            !repo
                .renew_document_cleanup_task_lock(claimed.task_id, Uuid::new_v4())
                .await
                .unwrap()
        );
        assert_eq!(
            repo.fail_document_cleanup_task(
                claimed.task_id,
                lock_token,
                "cleanup transient failure"
            )
            .await
            .unwrap(),
            DocumentCleanupTaskFailureOutcome::Requeued
        );

        let deletion_error = sqlx::query("select deletion_error from documents where id = $1")
            .bind(document_id)
            .fetch_one(repo.raw())
            .await
            .unwrap()
            .try_get::<Option<String>, _>("deletion_error")
            .unwrap();
        assert_eq!(deletion_error.as_deref(), Some("cleanup transient failure"));

        sqlx::query("update document_cleanup_tasks set available_at = now() where task_id = $1")
            .bind(claimed.task_id)
            .execute(repo.raw())
            .await
            .unwrap();
        let claimed = repo
            .claim_next_document_cleanup_task("cleanup-test-worker", Some(60))
            .await
            .unwrap()
            .expect("cleanup task should be claimed again");
        let lock_token = claimed.lock_token.expect("claim must return lock token");

        let parse_run_id = Uuid::new_v4();
        sqlx::query(
            r#"
            insert into document_parse_runs (
                run_id, org_id, notebook_id, document_id, status, backend_summary, artifact_path
            ) values ($1, $2, $3, $4, 'running', $5, $6)
            "#,
        )
        .bind(parse_run_id)
        .bind(org_id.into_uuid())
        .bind(notebook_id)
        .bind(document_id)
        .bind(serde_json::json!({"test": true}))
        .bind("artifact/key")
        .execute(repo.raw())
        .await
        .unwrap();
        sqlx::query(
            r#"
            insert into chunks (org_id, document_id, chunk_type, content, metadata, parse_run_id)
            values ($1, $2, 'body', 'cleanup body', '{}'::jsonb, $3)
            "#,
        )
        .bind(org_id.into_uuid())
        .bind(document_id)
        .bind(parse_run_id)
        .execute(repo.raw())
        .await
        .unwrap();
        sqlx::query(
            r#"
            insert into document_assets (
                asset_id, org_id, notebook_id, document_id, page, asset_kind,
                storage_path, mime_type, parser_backend, parse_run_id
            ) values ($1, $2, $3, $4, 1, 'image', 'safe/asset.png', 'image/png', 'test', $5)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(org_id.into_uuid())
        .bind(notebook_id)
        .bind(document_id)
        .bind(parse_run_id)
        .execute(repo.raw())
        .await
        .unwrap();
        sqlx::query(
            r#"
            insert into document_blocks (
                org_id, notebook_id, document_id, block_id, page, block_type, modality,
                text, parser_backend, parse_run_id
            ) values ($1, $2, $3, 'block-1', 1, 'paragraph', 'text', 'block text', 'test', $4)
            "#,
        )
        .bind(org_id.into_uuid())
        .bind(notebook_id)
        .bind(document_id)
        .bind(parse_run_id)
        .execute(repo.raw())
        .await
        .unwrap();
        sqlx::query(
            r#"
            insert into document_multimodal_chunks (
                chunk_id, org_id, notebook_id, document_id, page, context_text,
                normalized_text, parser_backend, parse_run_id
            ) values ($1, $2, $3, $4, 1, 'context', 'normalized', 'test', $5)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(org_id.into_uuid())
        .bind(notebook_id)
        .bind(document_id)
        .bind(parse_run_id)
        .execute(repo.raw())
        .await
        .unwrap();

        let wrong_parse_run_id = Uuid::new_v4();
        sqlx::query(
            r#"
            insert into document_parse_runs (
                run_id, org_id, notebook_id, document_id, status, backend_summary
            ) values ($1, $2, $3, $4, 'completed', '{}'::jsonb)
            "#,
        )
        .bind(wrong_parse_run_id)
        .bind(other_org_id.into_uuid())
        .bind(other_notebook_id)
        .bind(document_id)
        .execute(repo.raw())
        .await
        .unwrap();
        sqlx::query(
            r#"
            insert into chunks (org_id, document_id, chunk_type, content, metadata, parse_run_id)
            values ($1, $2, 'body', 'wrong tenant body', '{}'::jsonb, $3)
            "#,
        )
        .bind(other_org_id.into_uuid())
        .bind(document_id)
        .bind(wrong_parse_run_id)
        .execute(repo.raw())
        .await
        .unwrap();
        sqlx::query(
            r#"
            insert into document_assets (
                asset_id, org_id, notebook_id, document_id, page, asset_kind,
                storage_path, mime_type, parser_backend, parse_run_id
            ) values ($1, $2, $3, $4, 1, 'image', 'wrong/asset.png', 'image/png', 'test', $5)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(other_org_id.into_uuid())
        .bind(other_notebook_id)
        .bind(document_id)
        .bind(wrong_parse_run_id)
        .execute(repo.raw())
        .await
        .unwrap();
        sqlx::query(
            r#"
            insert into document_blocks (
                org_id, notebook_id, document_id, block_id, page, block_type, modality,
                text, parser_backend, parse_run_id
            ) values ($1, $2, $3, 'wrong-block-1', 1, 'paragraph', 'text', 'wrong block', 'test', $4)
            "#,
        )
        .bind(other_org_id.into_uuid())
        .bind(other_notebook_id)
        .bind(document_id)
        .bind(wrong_parse_run_id)
        .execute(repo.raw())
        .await
        .unwrap();
        sqlx::query(
            r#"
            insert into document_multimodal_chunks (
                chunk_id, org_id, notebook_id, document_id, page, context_text,
                normalized_text, parser_backend, parse_run_id
            ) values ($1, $2, $3, $4, 1, 'wrong context', 'wrong normalized', 'test', $5)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(other_org_id.into_uuid())
        .bind(other_notebook_id)
        .bind(document_id)
        .bind(wrong_parse_run_id)
        .execute(repo.raw())
        .await
        .unwrap();

        let targets = repo
            .get_document_cleanup_targets(&ctx, document_id, &claimed.payload)
            .await
            .unwrap()
            .unwrap();
        assert!(
            targets
                .asset_storage_paths
                .contains(&"safe/asset.png".to_string())
        );
        assert!(
            repo.cleanup_document_derived_rows(&ctx, document_id)
                .await
                .unwrap()
        );
        assert!(repo.mark_document_deleted(&ctx, document_id).await.unwrap());
        assert_eq!(
            repo.get_document_status(&ctx, document_id).await.unwrap(),
            Some(DocumentStatus::Deleted)
        );
        for table in [
            "chunks",
            "document_assets",
            "document_blocks",
            "document_multimodal_chunks",
            "document_parse_runs",
        ] {
            let sql = format!(
                "select count(*)::bigint as c from {table} where org_id = $1 and document_id = $2"
            );
            let count = sqlx::query(&sql)
                .bind(org_id.into_uuid())
                .bind(document_id)
                .fetch_one(repo.raw())
                .await
                .unwrap()
                .try_get::<i64, _>("c")
                .unwrap();
            assert_eq!(count, 0, "{table} should be cleaned for owning tenant");

            let wrong_count = sqlx::query(&sql)
                .bind(other_org_id.into_uuid())
                .bind(document_id)
                .fetch_one(repo.raw())
                .await
                .unwrap()
                .try_get::<i64, _>("c")
                .unwrap();
            assert_eq!(wrong_count, 1, "{table} wrong-tenant row should remain");
        }
        assert_eq!(
            repo.complete_document_cleanup_task(claimed.task_id, lock_token)
                .await
                .unwrap(),
            DocumentCleanupTaskCompletionOutcome::Completed
        );
        assert_eq!(
            repo.complete_document_cleanup_task(claimed.task_id, lock_token)
                .await
                .unwrap(),
            DocumentCleanupTaskCompletionOutcome::LeaseLost
        );
    }

    #[tokio::test]
    async fn cleanup_targets_reject_active_document_when_database_available() {
        let Some(database_url) = env::var("DATABASE_URL").ok() else {
            return;
        };
        let repo = PgAppRepository::connect(&database_url).await.unwrap();
        repo.migrate().await.unwrap();

        let org_id = OrgId::from(Uuid::new_v4());
        let user_id = Uuid::new_v4();
        let ctx = AuthContext::new(org_id, avrag_auth::SubjectKind::User)
            .with_actor_id(ActorId::new(user_id));
        let notebook = repo
            .create_notebook(
                &ctx,
                "active cleanup guard notebook",
                "active cleanup guard",
            )
            .await
            .unwrap();
        let notebook_id = Uuid::parse_str(&notebook.id).unwrap();
        let document = repo
            .create_document(&ctx, notebook_id, "active.txt", 42, "text/plain")
            .await
            .unwrap();
        let document_id = Uuid::parse_str(&document.id).unwrap();
        sqlx::query(
            r#"
            insert into document_assets (
                asset_id, org_id, notebook_id, document_id, page, asset_kind,
                storage_path, mime_type, parser_backend
            ) values ($1, $2, $3, $4, 1, 'image', 'must/not/delete.png', 'image/png', 'test')
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(org_id.into_uuid())
        .bind(notebook_id)
        .bind(document_id)
        .execute(repo.raw())
        .await
        .unwrap();

        let payload = serde_json::json!({"object_path": "must/not/delete.txt"});
        assert!(
            repo.get_document_cleanup_targets(&ctx, document_id, &payload)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            !repo
                .cleanup_document_derived_rows(&ctx, document_id)
                .await
                .unwrap()
        );
        let remaining = sqlx::query(
            "select count(*)::bigint as c from document_assets where org_id = $1 and document_id = $2",
        )
        .bind(org_id.into_uuid())
        .bind(document_id)
        .fetch_one(repo.raw())
        .await
        .unwrap()
        .try_get::<i64, _>("c")
        .unwrap();
        assert_eq!(remaining, 1);
    }

    #[tokio::test]
    async fn ingestion_side_effect_guard_requires_current_lease_and_non_deleting_document_when_database_available()
     {
        let Some(database_url) = env::var("DATABASE_URL").ok() else {
            return;
        };
        let repo = PgAppRepository::connect(&database_url).await.unwrap();
        repo.migrate().await.unwrap();

        let org_id = OrgId::from(Uuid::new_v4());
        let user_id = Uuid::new_v4();
        let ctx = AuthContext::new(org_id, avrag_auth::SubjectKind::User)
            .with_actor_id(ActorId::new(user_id));
        let notebook = repo
            .create_notebook(&ctx, "ingestion guard notebook", "ingestion guard")
            .await
            .unwrap();
        let document = repo
            .create_document(
                &ctx,
                Uuid::parse_str(&notebook.id).unwrap(),
                "ingestion-guard.txt",
                42,
                "text/plain",
            )
            .await
            .unwrap();
        let document_id = Uuid::parse_str(&document.id).unwrap();
        let task = ingestion::build_ingest_task(
            org_id.to_string(),
            notebook.id.clone(),
            document.id.clone(),
            Some(user_id.to_string()),
            ingestion::IngestDocumentPayload {
                source_uri: "s3://bucket/org/notebook/doc/ingestion-guard.txt".to_string(),
                object_path: "org/notebook/doc/ingestion-guard.txt".to_string(),
                mime_type: "text/plain".to_string(),
                filename: "ingestion-guard.txt".to_string(),
                file_size: 42,
            },
        );
        assert!(repo.enqueue_ingestion_task(&task).await.unwrap());
        let task_id = Uuid::parse_str(&task.task_id).unwrap();
        let lock_token = Uuid::new_v4();
        sqlx::query(
            r#"
            update ingestion_tasks
            set status = 'processing', locked_at = now(), locked_by = 'guard-test',
                lock_token = $2, attempt_count = attempt_count + 1, updated_at = now()
            where task_id = $1
            "#,
        )
        .bind(task_id)
        .bind(lock_token)
        .execute(repo.raw())
        .await
        .unwrap();

        let lock_token_string = lock_token.to_string();
        assert!(
            repo.document_allows_ingestion_side_effects(
                &ctx,
                document_id,
                &task.task_id,
                Some(&lock_token_string),
            )
            .await
            .unwrap()
        );
        assert!(
            !repo
                .document_allows_ingestion_side_effects(
                    &ctx,
                    document_id,
                    &task.task_id,
                    Some(&Uuid::new_v4().to_string()),
                )
                .await
                .unwrap()
        );
        assert_eq!(
            repo.delete_document(&ctx, document_id).await.unwrap(),
            DocumentDeletionOutcome::Queued {
                task_inserted: true
            }
        );
        assert!(
            !repo
                .document_allows_ingestion_side_effects(
                    &ctx,
                    document_id,
                    &task.task_id,
                    Some(&lock_token_string),
                )
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn generic_status_update_rejects_deleting_and_deleted_when_database_available() {
        let Some(database_url) = env::var("DATABASE_URL").ok() else {
            return;
        };
        let repo = PgAppRepository::connect(&database_url).await.unwrap();
        repo.migrate().await.unwrap();

        let org_id = OrgId::from(Uuid::new_v4());
        let user_id = Uuid::new_v4();
        let ctx = AuthContext::new(org_id, avrag_auth::SubjectKind::User)
            .with_actor_id(ActorId::new(user_id));

        let notebook = repo
            .create_notebook(&ctx, "status guard test notebook", "status guard test")
            .await
            .unwrap();
        let document = repo
            .create_document(
                &ctx,
                Uuid::parse_str(&notebook.id).unwrap(),
                "status-guard.txt",
                42,
                "text/plain",
            )
            .await
            .unwrap();
        let document_id = Uuid::parse_str(&document.id).unwrap();

        assert!(
            !repo
                .update_document(
                    &ctx,
                    document_id,
                    None,
                    None,
                    Some(DocumentStatus::Deleting)
                )
                .await
                .unwrap()
        );
        assert!(
            !repo
                .set_document_status(&ctx, document_id, DocumentStatus::Deleted)
                .await
                .unwrap()
        );
        assert_eq!(
            repo.get_document_status(&ctx, document_id).await.unwrap(),
            Some(DocumentStatus::Pending)
        );
        assert_eq!(
            repo.count_document_cleanup_tasks_for_document(&ctx, document_id)
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn update_document_summary_overwrites_existing_summary_when_database_available() {
        let Some(database_url) = env::var("DATABASE_URL").ok() else {
            return;
        };
        let repo = PgAppRepository::connect(&database_url).await.unwrap();
        repo.migrate().await.unwrap();

        let org_id = OrgId::from(Uuid::new_v4());
        let user_id = Uuid::new_v4();
        let ctx = AuthContext::new(org_id, avrag_auth::SubjectKind::User)
            .with_actor_id(ActorId::new(user_id));

        let notebook = repo
            .create_notebook(&ctx, "summary test notebook", "summary test")
            .await
            .unwrap();
        let document = repo
            .create_document(
                &ctx,
                Uuid::parse_str(&notebook.id).unwrap(),
                "summary-test.txt",
                42,
                "text/plain",
            )
            .await
            .unwrap();
        let document_id = Uuid::parse_str(&document.id).unwrap();

        repo.store_document_body(&ctx, document_id, "First line. Second line. Third line.")
            .await
            .unwrap();
        let summary_output = common::SummaryOutput {
            summary_text: "LLM upgraded summary".to_string(),
            summary_metadata: common::SummaryMetadata {
                doc_id: document_id.to_string(),
                filename: "summary-test.txt".to_string(),
                docname: "summary test".to_string(),
                language: "en".to_string(),
                domain: common::Domain::Unknown,
                genre: common::Genre::Unknown,
                era: common::Era::Contemporary,
                author: None,
                publication_date: None,
            },
        };
        repo.update_document_summary(&ctx, document_id, &summary_output, None, None)
            .await
            .unwrap();

        let preview = repo
            .get_parsed_preview(&ctx, document_id, 0, 10)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(preview.summary.as_deref(), Some("LLM upgraded summary"));
    }

    // -----------------------------------------------------------------------
    // Tool results persistence roundtrip
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn chat_message_tool_results_roundtrip_when_database_available() {
        let Some(database_url) = env::var("DATABASE_URL").ok() else {
            return;
        };
        let repo = PgAppRepository::connect(&database_url).await.unwrap();
        repo.migrate().await.unwrap();

        let org_id = OrgId::from(Uuid::new_v4());
        let _org_uuid = org_id.into_uuid();
        let user_id = Uuid::new_v4();
        let ctx = AuthContext::new(org_id, avrag_auth::SubjectKind::User)
            .with_actor_id(ActorId::new(user_id));

        let notebook = repo
            .create_notebook(&ctx, "tool-results-test", "tool results test")
            .await
            .unwrap();
        let notebook_id = Uuid::parse_str(&notebook.id).unwrap();

        let session = repo
            .create_session(
                &ctx,
                notebook_id,
                Some("test-session-title"),
                "rag",
            )
            .await
            .unwrap();
        let session_id = Uuid::parse_str(&session.id).unwrap();

        let tool_results: Vec<contracts::ToolResult> = vec![
            contracts::ToolResult {
                tool: "calculator".to_string(),
                version: "1.0".to_string(),
                status: contracts::ToolStatus::Ok,
                data: Some(serde_json::json!({"result": 42.0, "expression": "6*7"})),
                trace: None,
            },
            contracts::ToolResult {
                tool: "code_interpreter".to_string(),
                version: "1.0".to_string(),
                status: contracts::ToolStatus::Error,
                data: Some(serde_json::json!({"error": "SyntaxError"})),
                trace: None,
            },
        ];

        let message_id = repo
            .append_chat_turn(
                &ctx,
                session_id,
                ChatTurn {
                    user_content: "Calculate something",
                    assistant_content: "Here are the results.",
                    assistant_answer_blocks: &[],
                    agent_type: "chat",
                    citations: &[],
                    tool_results: &tool_results,
                    user_turn_metadata: None,
                    user_resolved_query: None,
                },
            )
            .await
            .unwrap();
        assert!(message_id > 0);

        let messages = repo.list_messages(&ctx, session_id).await.unwrap();
        let assistant_message = messages
            .iter()
            .find(|m| m.role == "assistant")
            .expect("assistant message exists");

        assert_eq!(assistant_message.tool_results.len(), 2);
        assert_eq!(assistant_message.tool_results[0].tool, "calculator");
        assert_eq!(assistant_message.tool_results[0].status, contracts::ToolStatus::Ok);
        assert_eq!(
            assistant_message.tool_results[0].data.as_ref().unwrap()["result"],
            42.0
        );
        assert_eq!(assistant_message.tool_results[1].tool, "code_interpreter");
        assert_eq!(assistant_message.tool_results[1].status, contracts::ToolStatus::Error);
        assert_eq!(
            assistant_message.tool_results[1].data.as_ref().unwrap()["error"],
            "SyntaxError"
        );
    }

    #[tokio::test]
    async fn chat_message_turn_metadata_roundtrip_when_database_available() {
        let Some(database_url) = env::var("DATABASE_URL").ok() else {
            return;
        };
        let repo = PgAppRepository::connect(&database_url).await.unwrap();
        repo.migrate().await.unwrap();

        let org_id = OrgId::from(Uuid::new_v4());
        let ctx = AuthContext::new(org_id, avrag_auth::SubjectKind::User)
            .with_actor_id(ActorId::new(Uuid::new_v4()));

        let notebook = repo
            .create_notebook(&ctx, "turn-metadata-test", "turn metadata test")
            .await
            .unwrap();
        let notebook_id = Uuid::parse_str(&notebook.id).unwrap();
        let session = repo
            .create_session(&ctx, notebook_id, Some("meta-session"), "rag")
            .await
            .unwrap();
        let session_id = Uuid::parse_str(&session.id).unwrap();

        let metadata = serde_json::json!({
            "query_resolution": {
                "raw_query": "Who wrote it?",
                "resolved_query": "Who wrote Antifragile?",
                "slots": ["pronoun"],
                "method": "heuristic"
            }
        });

        let message_id = repo
            .append_chat_turn(
                &ctx,
                session_id,
                ChatTurn {
                    user_content: "Who wrote it?",
                    assistant_content: "Taleb.",
                    assistant_answer_blocks: &[],
                    agent_type: "rag",
                    citations: &[],
                    tool_results: &[],
                    user_turn_metadata: Some(metadata),
                    user_resolved_query: Some("Who wrote Antifragile?"),
                },
            )
            .await
            .unwrap();

        let messages = repo.list_messages(&ctx, session_id).await.unwrap();
        let user_row = messages
            .iter()
            .find(|m| m.role == "user")
            .expect("user row");
        assert_eq!(user_row.content, "Who wrote it?");
        let stored_meta = user_row
            .turn_metadata
            .as_ref()
            .expect("turn_metadata should roundtrip");
        assert_eq!(
            stored_meta["query_resolution"]["resolved_query"],
            "Who wrote Antifragile?"
        );
        assert_eq!(
            user_row.resolved_query.as_deref(),
            Some("Who wrote Antifragile?")
        );
        assert!(message_id > 0);
    }

    #[tokio::test]
    async fn search_conversation_history_notebook_scope_spans_sessions_when_database_available() {
        let Some(database_url) = env::var("DATABASE_URL").ok() else {
            return;
        };
        let repo = PgAppRepository::connect(&database_url).await.unwrap();
        repo.migrate().await.unwrap();

        let org_id = OrgId::from(Uuid::new_v4());
        let user_id = Uuid::new_v4();
        let ctx = AuthContext::new(org_id, avrag_auth::SubjectKind::User)
            .with_actor_id(ActorId::new(user_id));

        let notebook = repo
            .create_notebook(&ctx, "memory-search", "memory search test")
            .await
            .unwrap();
        let notebook_id = Uuid::parse_str(&notebook.id).unwrap();

        let session_a = repo
            .create_session(&ctx, notebook_id, Some("session-a"), "rag")
            .await
            .unwrap();
        let session_a_id = Uuid::parse_str(&session_a.id).unwrap();
        let session_b = repo
            .create_session(&ctx, notebook_id, Some("session-b"), "rag")
            .await
            .unwrap();
        let session_b_id = Uuid::parse_str(&session_b.id).unwrap();

        repo.append_chat_turn(
            &ctx,
            session_a_id,
            ChatTurn {
                user_content: "What is antifragility?",
                assistant_content: "Antifragility gains from disorder.",
                assistant_answer_blocks: &[],
                agent_type: "rag",
                citations: &[],
                tool_results: &[],
                user_turn_metadata: None,
                user_resolved_query: Some("What is antifragility?"),
            },
        )
        .await
        .unwrap();

        repo.append_chat_turn(
            &ctx,
            session_b_id,
            ChatTurn {
                user_content: "Open a second session.",
                assistant_content: "Sure.",
                assistant_answer_blocks: &[],
                agent_type: "rag",
                citations: &[],
                tool_results: &[],
                user_turn_metadata: None,
                user_resolved_query: None,
            },
        )
        .await
        .unwrap();

        let tokens_row: Option<(Option<String>,)> = sqlx::query_as(
            "SELECT search_tokens FROM chat_messages WHERE session_id = $1 AND role = 'user' ORDER BY id DESC LIMIT 1",
        )
        .bind(session_a_id)
        .fetch_optional(repo.raw())
        .await
        .unwrap();
        assert!(
            tokens_row
                .and_then(|(t,)| t)
                .is_some_and(|t| !t.trim().is_empty()),
            "search_tokens should be populated on insert"
        );

        let hits = repo
            .search_conversation_history(
                &ctx,
                session_b_id,
                "antifragility",
                ConversationHistoryScope::Notebook,
                10,
                &[],
            )
            .await
            .unwrap();

        assert!(
            hits.iter().any(|hit| hit.session_id == session_a_id),
            "notebook scope should return messages from another session in the same notebook"
        );
    }

    #[tokio::test]
    async fn search_sessions_matches_assistant_message_body_when_database_available() {
        let Some(database_url) = env::var("DATABASE_URL").ok() else {
            return;
        };
        let repo = PgAppRepository::connect(&database_url).await.unwrap();
        repo.migrate().await.unwrap();

        let org_id = OrgId::from(Uuid::new_v4());
        let user_id = Uuid::new_v4();
        let ctx = AuthContext::new(org_id, avrag_auth::SubjectKind::User)
            .with_actor_id(ActorId::new(user_id));

        let notebook = repo
            .create_notebook(&ctx, "session-search", "session search test")
            .await
            .unwrap();
        let notebook_id = Uuid::parse_str(&notebook.id).unwrap();

        let session = repo
            .create_session(&ctx, notebook_id, Some("generic title"), "rag")
            .await
            .unwrap();
        let session_id = Uuid::parse_str(&session.id).unwrap();

        repo.append_chat_turn(
            &ctx,
            session_id,
            ChatTurn {
                user_content: "Tell me something.",
                assistant_content: "The secret roadmap keyword is zephyrneedle2026.",
                assistant_answer_blocks: &[],
                agent_type: "rag",
                citations: &[],
                tool_results: &[],
                user_turn_metadata: None,
                user_resolved_query: None,
            },
        )
        .await
        .unwrap();

        let pattern = "%zephyrneedle2026%";
        let matches = repo.search_sessions(&ctx, pattern).await.unwrap();
        assert!(
            matches.iter().any(|item| item.id == session.id),
            "search_sessions should match assistant message FTS, not only session title"
        );
    }

    #[tokio::test]
    async fn search_conversation_history_matches_assistant_message_when_database_available() {
        let Some(database_url) = env::var("DATABASE_URL").ok() else {
            return;
        };
        let repo = PgAppRepository::connect(&database_url).await.unwrap();
        repo.migrate().await.unwrap();

        let org_id = OrgId::from(Uuid::new_v4());
        let user_id = Uuid::new_v4();
        let ctx = AuthContext::new(org_id, avrag_auth::SubjectKind::User)
            .with_actor_id(ActorId::new(user_id));

        let notebook = repo
            .create_notebook(&ctx, "assistant-history", "assistant history test")
            .await
            .unwrap();
        let notebook_id = Uuid::parse_str(&notebook.id).unwrap();

        let session_a = repo
            .create_session(&ctx, notebook_id, Some("session-a"), "rag")
            .await
            .unwrap();
        let session_a_id = Uuid::parse_str(&session_a.id).unwrap();
        let session_b = repo
            .create_session(&ctx, notebook_id, Some("session-b"), "rag")
            .await
            .unwrap();
        let session_b_id = Uuid::parse_str(&session_b.id).unwrap();

        repo.append_chat_turn(
            &ctx,
            session_a_id,
            ChatTurn {
                user_content: "Explain a concept.",
                assistant_content: "Antifragility gains from volatility and stressors.",
                assistant_answer_blocks: &[],
                agent_type: "rag",
                citations: &[],
                tool_results: &[],
                user_turn_metadata: None,
                user_resolved_query: None,
            },
        )
        .await
        .unwrap();

        repo.append_chat_turn(
            &ctx,
            session_b_id,
            ChatTurn {
                user_content: "Another topic.",
                assistant_content: "Sure.",
                assistant_answer_blocks: &[],
                agent_type: "rag",
                citations: &[],
                tool_results: &[],
                user_turn_metadata: None,
                user_resolved_query: None,
            },
        )
        .await
        .unwrap();

        let hits = repo
            .search_conversation_history(
                &ctx,
                session_b_id,
                "antifragility",
                ConversationHistoryScope::Notebook,
                10,
                &[],
            )
            .await
            .unwrap();

        assert!(
            hits.iter().any(|hit| hit.session_id == session_a_id && hit.role == "assistant"),
            "conversation_history should match assistant message body across sessions"
        );
    }

    #[tokio::test]
    async fn get_notebook_returns_none_for_other_org_when_database_available() {
        let Some(database_url) = env::var("DATABASE_URL").ok() else {
            return;
        };
        let repo = PgAppRepository::connect(&database_url).await.unwrap();
        repo.migrate().await.unwrap();

        let org_a = OrgId::from(Uuid::new_v4());
        let org_b = OrgId::from(Uuid::new_v4());
        let ctx_a = AuthContext::new(org_a, avrag_auth::SubjectKind::User)
            .with_actor_id(ActorId::new(Uuid::new_v4()));
        let ctx_b = AuthContext::new(org_b, avrag_auth::SubjectKind::User)
            .with_actor_id(ActorId::new(Uuid::new_v4()));

        let notebook = repo
            .create_notebook(&ctx_a, "org-a notebook", "isolation test")
            .await
            .unwrap();
        let notebook_id = Uuid::parse_str(&notebook.id).unwrap();

        let fetched = repo.get_notebook(&ctx_b, notebook_id).await.unwrap();
        assert!(
            fetched.is_none(),
            "org B must not read org A's notebook via get_notebook"
        );
    }
}
