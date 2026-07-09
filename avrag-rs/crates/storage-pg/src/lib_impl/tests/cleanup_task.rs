use super::support::*;

#[tokio::test]
async fn document_cleanup_task_claim_fail_complete_and_db_cleanup_when_database_available() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository { pool: __bootstrap.pool.clone() };
    repo.bootstrap().migrate().await.unwrap();

    let org_id = OrgId::from(Uuid::new_v4());
    let user_id = Uuid::new_v4();
    let ctx = AuthContext::new(org_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(user_id));
    let other_org_id = OrgId::from(Uuid::new_v4());
    let other_user_id = Uuid::new_v4();
    let other_ctx = AuthContext::new(other_org_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(other_user_id));
    let other_notebook = repo
        .bootstrap().create_workspace(
            &other_ctx,
            "cleanup other tenant notebook",
            "cleanup other tenant",
        )
        .await
        .unwrap();
    let other_workspace_id = Uuid::parse_str(&other_notebook.id).unwrap();
    let notebook = repo
        .bootstrap().create_workspace(&ctx, "cleanup task test notebook", "cleanup task test")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&notebook.id).unwrap();
    let document = repo
        .bootstrap().create_document(&ctx, workspace_id, "cleanup-me.txt", 42, "text/plain")
        .await
        .unwrap();
    let document_id = Uuid::parse_str(&document.id).unwrap();
    assert_eq!(
        repo.documents().delete_document(&ctx, document_id).await.unwrap(),
        DocumentDeletionOutcome::Queued {
            task_inserted: true
        }
    );

    let mut claimed = None;
    for _ in 0..20 {
        let next = repo
            .ingestion_queue().claim_next_document_cleanup_task("cleanup-test-worker", Some(60))
            .await
            .unwrap();
        let Some(task) = next else { break };
        if task.document_id == document_id {
            claimed = Some(task);
            break;
        }
    }
    let claimed = claimed.expect("cleanup task for our document should be claimed");
    assert_eq!(claimed.org_id, org_id.into_uuid());
    assert_eq!(claimed.workspace_id, workspace_id);
    assert_eq!(claimed.document_id, document_id);
    let lock_token = claimed.lock_token.expect("claim must return lock token");
    assert!(
        repo.ingestion_queue().renew_document_cleanup_task_lock(claimed.task_id, lock_token)
            .await
            .unwrap()
    );
    assert!(
        repo.chunks().document_cleanup_task_lease_is_current(claimed.task_id, lock_token)
            .await
            .unwrap()
    );
    assert!(
        !repo
            .chunks().document_cleanup_task_lease_is_current(claimed.task_id, Uuid::new_v4())
            .await
            .unwrap()
    );
    assert!(
        !repo
            .ingestion_queue().renew_document_cleanup_task_lock(claimed.task_id, Uuid::new_v4())
            .await
            .unwrap()
    );
    assert_eq!(
        repo.ingestion_queue().fail_document_cleanup_task(
            claimed.task_id,
            lock_token,
            "cleanup transient failure"
        )
        .await
        .unwrap(),
        DocumentCleanupTaskFailureOutcome::Requeued
    );

    let deletion_error = {
        let mut tx = repo.raw().begin().await.unwrap();
        sqlx::query("select set_config('app.current_org', $1, true)")
            .bind(org_id.into_uuid().to_string())
            .execute(tx.as_mut())
            .await
            .unwrap();
        let row = sqlx::query("select deletion_error from documents where id = $1")
            .bind(document_id)
            .fetch_one(tx.as_mut())
            .await
            .unwrap();
        tx.commit().await.unwrap();
        row.try_get::<Option<String>, _>("deletion_error").unwrap()
    };
    assert_eq!(deletion_error.as_deref(), Some("cleanup transient failure"));

    {
        let mut tx = repo.raw().begin().await.unwrap();
        sqlx::query("select set_config('app.document_cleanup_worker', 'true', true)")
            .execute(tx.as_mut())
            .await
            .unwrap();
        sqlx::query("update document_cleanup_tasks set available_at = now() where task_id = $1")
            .bind(claimed.task_id)
            .execute(tx.as_mut())
            .await
            .unwrap();
        tx.commit().await.unwrap();
    }
    let mut claimed_again = None;
    for _ in 0..20 {
        let next = repo
            .ingestion_queue()
            .claim_next_document_cleanup_task("cleanup-test-worker", Some(60))
            .await
            .unwrap();
        let Some(task) = next else {
            break;
        };
        if task.task_id == claimed.task_id {
            claimed_again = Some(task);
            break;
        }
    }
    let claimed = claimed_again.expect("cleanup task should be claimed again");
    let lock_token = claimed.lock_token.expect("claim must return lock token");

    let parse_run_id = Uuid::new_v4();
    let mut seed_tx = repo.raw().begin().await.unwrap();
    sqlx::query("select set_config('app.current_org', $1, true)")
        .bind(org_id.into_uuid().to_string())
        .execute(seed_tx.as_mut())
        .await
        .unwrap();
    sqlx::query(
        r#"
        insert into document_parse_runs (
            run_id, org_id, workspace_id, document_id, status, backend_summary, artifact_path
        ) values ($1, $2, $3, $4, 'running', $5, $6)
        "#,
    )
    .bind(parse_run_id)
    .bind(org_id.into_uuid())
    .bind(workspace_id)
    .bind(document_id)
    .bind(serde_json::json!({"test": true}))
    .bind("artifact/key")
    .execute(seed_tx.as_mut())
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
    .execute(seed_tx.as_mut())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into document_assets (
            asset_id, org_id, workspace_id, document_id, page, asset_kind,
            storage_path, mime_type, parser_backend, parse_run_id
        ) values ($1, $2, $3, $4, 1, 'image', 'safe/asset.png', 'image/png', 'test', $5)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(org_id.into_uuid())
    .bind(workspace_id)
    .bind(document_id)
    .bind(parse_run_id)
    .execute(seed_tx.as_mut())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into document_blocks (
            org_id, workspace_id, document_id, block_id, page, block_type, modality,
            text, parser_backend, parse_run_id
        ) values ($1, $2, $3, 'block-1', 1, 'paragraph', 'text', 'block text', 'test', $4)
        "#,
    )
    .bind(org_id.into_uuid())
    .bind(workspace_id)
    .bind(document_id)
    .bind(parse_run_id)
    .execute(seed_tx.as_mut())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into document_multimodal_chunks (
            chunk_id, org_id, workspace_id, document_id, page, context_text,
            normalized_text, parser_backend, parse_run_id
        ) values ($1, $2, $3, $4, 1, 'context', 'normalized', 'test', $5)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(org_id.into_uuid())
    .bind(workspace_id)
    .bind(document_id)
    .bind(parse_run_id)
    .execute(seed_tx.as_mut())
    .await
    .unwrap();
    seed_tx.commit().await.unwrap();

    let wrong_parse_run_id = Uuid::new_v4();
    let mut other_tx = repo.raw().begin().await.unwrap();
    sqlx::query("select set_config('app.current_org', $1, true)")
        .bind(other_org_id.into_uuid().to_string())
        .execute(other_tx.as_mut())
        .await
        .unwrap();
    sqlx::query(
        r#"
        insert into document_parse_runs (
            run_id, org_id, workspace_id, document_id, status, backend_summary
        ) values ($1, $2, $3, $4, 'completed', '{}'::jsonb)
        "#,
    )
    .bind(wrong_parse_run_id)
    .bind(other_org_id.into_uuid())
    .bind(other_workspace_id)
    .bind(document_id)
    .execute(other_tx.as_mut())
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
    .execute(other_tx.as_mut())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into document_assets (
            asset_id, org_id, workspace_id, document_id, page, asset_kind,
            storage_path, mime_type, parser_backend, parse_run_id
        ) values ($1, $2, $3, $4, 1, 'image', 'wrong/asset.png', 'image/png', 'test', $5)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(other_org_id.into_uuid())
    .bind(other_workspace_id)
    .bind(document_id)
    .bind(wrong_parse_run_id)
    .execute(other_tx.as_mut())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into document_blocks (
            org_id, workspace_id, document_id, block_id, page, block_type, modality,
            text, parser_backend, parse_run_id
        ) values ($1, $2, $3, 'wrong-block-1', 1, 'paragraph', 'text', 'wrong block', 'test', $4)
        "#,
    )
    .bind(other_org_id.into_uuid())
    .bind(other_workspace_id)
    .bind(document_id)
    .bind(wrong_parse_run_id)
    .execute(other_tx.as_mut())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into document_multimodal_chunks (
            chunk_id, org_id, workspace_id, document_id, page, context_text,
            normalized_text, parser_backend, parse_run_id
        ) values ($1, $2, $3, $4, 1, 'wrong context', 'wrong normalized', 'test', $5)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(other_org_id.into_uuid())
    .bind(other_workspace_id)
    .bind(document_id)
    .bind(wrong_parse_run_id)
    .execute(other_tx.as_mut())
    .await
    .unwrap();
    other_tx.commit().await.unwrap();

    let targets = repo
        .chunks().get_document_cleanup_targets(&ctx, document_id, &claimed.payload)
        .await
        .unwrap()
        .unwrap();
    assert!(
        targets
            .asset_storage_paths
            .contains(&"safe/asset.png".to_string())
    );
    assert!(
        repo.chunks().cleanup_document_derived_rows(&ctx, document_id)
            .await
            .unwrap()
    );
    assert!(repo.chunks().mark_document_deleted(&ctx, document_id).await.unwrap());
    assert_eq!(
        repo.documents().get_document_status(&ctx, document_id).await.unwrap(),
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
        let mut tx = repo.raw().begin().await.unwrap();
        sqlx::query("select set_config('app.current_org', $1, true)")
            .bind(org_id.into_uuid().to_string())
            .execute(tx.as_mut())
            .await
            .unwrap();
        let count = sqlx::query(&sql)
            .bind(org_id.into_uuid())
            .bind(document_id)
            .fetch_one(tx.as_mut())
            .await
            .unwrap()
            .try_get::<i64, _>("c")
            .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(count, 0, "{table} should be cleaned for owning tenant");

        let mut tx = repo.raw().begin().await.unwrap();
        sqlx::query("select set_config('app.current_org', $1, true)")
            .bind(other_org_id.into_uuid().to_string())
            .execute(tx.as_mut())
            .await
            .unwrap();
        let wrong_count = sqlx::query(&sql)
            .bind(other_org_id.into_uuid())
            .bind(document_id)
            .fetch_one(tx.as_mut())
            .await
            .unwrap()
            .try_get::<i64, _>("c")
            .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(wrong_count, 1, "{table} wrong-tenant row should remain");
    }
    assert_eq!(
        repo.ingestion_queue().complete_document_cleanup_task(claimed.task_id, lock_token)
            .await
            .unwrap(),
        DocumentCleanupTaskCompletionOutcome::Completed
    );
    assert_eq!(
        repo.ingestion_queue().complete_document_cleanup_task(claimed.task_id, lock_token)
            .await
            .unwrap(),
        DocumentCleanupTaskCompletionOutcome::LeaseLost
    );
}

