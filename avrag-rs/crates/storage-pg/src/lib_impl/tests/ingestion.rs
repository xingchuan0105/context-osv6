use super::support::*;

#[tokio::test]
async fn renew_ingestion_task_lock_matches_processing_task_lease_when_database_available() {
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

    let notebook = repo
        .bootstrap().create_notebook(&ctx, "lease renewal test notebook", "lease renewal test")
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

    let org_id = OrgId::from(Uuid::new_v4());
    let user_id = Uuid::new_v4();
    let ctx = AuthContext::new(org_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(user_id));
    let notebook = repo
        .bootstrap().create_notebook(&ctx, "ingestion guard notebook", "ingestion guard")
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

