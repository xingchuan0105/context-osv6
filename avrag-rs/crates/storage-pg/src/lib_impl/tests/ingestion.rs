use super::support::*;

/// Claim is cross-tenant scheduling: without super_admin GUC, forced RLS hides other
/// owners' queued tasks (Journey 2026-07-13). claim_next must still see them.
#[tokio::test]
async fn claim_next_ingestion_task_sees_cross_owner_queue_under_forced_rls() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository {
        pool: __bootstrap.pool.clone(),
    };
    repo.bootstrap().migrate().await.unwrap();

    // B2C personal account: owner == actor (distinct actor insert hits users RLS WITH CHECK).
    let owner_uuid = Uuid::new_v4();
    let owner_user_id = UserId::from(owner_uuid);
    let ctx = AuthContext::new(owner_user_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(owner_uuid));

    let workspace = repo
        .bootstrap()
        .create_workspace(&ctx, "claim rls notebook", "claim rls")
        .await
        .unwrap();
    let document = repo
        .bootstrap()
        .create_document(
            &ctx,
            Uuid::parse_str(&workspace.id).unwrap(),
            "claim-rls.txt",
            42,
            "text/plain",
        )
        .await
        .unwrap();

    let task = ingestion::build_ingest_task(
        owner_user_id.to_string(),
        workspace.id.clone(),
        document.id.clone(),
        Some(owner_uuid.to_string()),
        ingestion::IngestDocumentPayload {
            source_uri: "s3://bucket/org/notebook/doc/claim-rls.txt".to_string(),
            object_path: "org/notebook/doc/claim-rls.txt".to_string(),
            mime_type: "text/plain".to_string(),
            filename: "claim-rls.txt".to_string(),
            file_size: 42,
        },
    );
    assert!(
        repo.ingestion_queue()
            .enqueue_ingestion_task(&task)
            .await
            .unwrap()
    );
    let task_id = Uuid::parse_str(&task.task_id).unwrap();

    // No GUC: forced RLS must hide the row (same as bare worker pool before fix).
    let visible_without_guc: i64 = sqlx::query_scalar(
        "select count(*)::bigint from ingestion_tasks where task_id = $1",
    )
    .bind(task_id)
    .fetch_one(repo.raw())
    .await
    .unwrap();
    assert_eq!(
        visible_without_guc, 0,
        "ingestion_tasks must be invisible without app.current_role under forced RLS"
    );

    let queue_group = std::env::var("AVRAG_INGESTION_QUEUE_GROUP")
        .unwrap_or_else(|_| "default".to_string());
    let claimed = repo
        .ingestion_queue()
        .claim_next_ingestion_task("claim-rls-test-worker", &queue_group)
        .await
        .unwrap();
    let claimed = claimed.expect("claim_next must elevate and see cross-owner queued task");
    assert_eq!(claimed.task_id, task.task_id);
    assert_eq!(claimed.document_id, document.id);

    let lock = claimed
        .lock_token
        .as_deref()
        .expect("claimed task has lock_token");
    assert_eq!(
        repo.ingestion_queue()
            .complete_ingestion_task(&task.task_id, Some(lock))
            .await
            .unwrap(),
        TaskCompletionOutcome::Completed
    );
}

#[tokio::test]
async fn renew_ingestion_task_lock_matches_processing_task_lease_when_database_available() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository { pool: __bootstrap.pool.clone() };
    repo.bootstrap().migrate().await.unwrap();

    let owner_user_id = UserId::from(Uuid::new_v4());
    let user_id = Uuid::new_v4();
    let ctx = AuthContext::new(owner_user_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(user_id));

    let notebook = repo
        .bootstrap().create_workspace(&ctx, "lease renewal test notebook", "lease renewal test")
        .await
        .unwrap();
    let document = repo
        .bootstrap().create_document(
            &ctx,
            Uuid::parse_str(&notebook.id).unwrap(),
            "lease-renewal-test.txt",
            42,
            "text/plain",
        )
        .await
        .unwrap();

    let task = ingestion::build_ingest_task(
        owner_user_id.to_string(),
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
    assert!(repo.ingestion_queue().enqueue_ingestion_task(&task).await.unwrap());
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
        repo.ingestion_queue().renew_ingestion_task_lock(&task.task_id, &lock_token)
            .await
            .unwrap()
    );
    assert!(
        !repo
            .ingestion_queue().renew_ingestion_task_lock(&task.task_id, &Uuid::new_v4().to_string())
            .await
            .unwrap()
    );
    assert_eq!(
        repo.ingestion_queue().complete_ingestion_task(&task.task_id, Some(&lock_token))
            .await
            .unwrap(),
        TaskCompletionOutcome::Completed
    );
}

#[tokio::test]
async fn ingestion_side_effect_guard_requires_current_lease_and_non_deleting_document_when_database_available()
 {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository { pool: __bootstrap.pool.clone() };
    repo.bootstrap().migrate().await.unwrap();

    let owner_user_id = UserId::from(Uuid::new_v4());
    let user_id = Uuid::new_v4();
    let ctx = AuthContext::new(owner_user_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(user_id));
    let notebook = repo
        .bootstrap().create_workspace(&ctx, "ingestion guard notebook", "ingestion guard")
        .await
        .unwrap();
    let document = repo
        .bootstrap().create_document(
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
        owner_user_id.to_string(),
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
    assert!(repo.ingestion_queue().enqueue_ingestion_task(&task).await.unwrap());
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
        repo.documents().document_allows_ingestion_side_effects(
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
            .documents().document_allows_ingestion_side_effects(
                &ctx,
                document_id,
                &task.task_id,
                Some(&Uuid::new_v4().to_string()),
            )
            .await
            .unwrap()
    );
    assert_eq!(
        repo.documents().delete_document(&ctx, document_id).await.unwrap(),
        DocumentDeletionOutcome::Queued {
            task_inserted: true
        }
    );
    assert!(
        !repo
            .documents().document_allows_ingestion_side_effects(
                &ctx,
                document_id,
                &task.task_id,
                Some(&lock_token_string),
            )
            .await
            .unwrap()
    );
}

