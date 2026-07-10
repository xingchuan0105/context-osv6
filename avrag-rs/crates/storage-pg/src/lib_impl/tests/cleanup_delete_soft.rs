use super::support::*;

#[tokio::test]
async fn delete_document_soft_deletes_and_enqueues_cleanup_once_when_database_available() {
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
        .bootstrap().create_workspace(&ctx, "soft delete test notebook", "soft delete test")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&notebook.id).unwrap();
    let document = repo
        .bootstrap().create_document(&ctx, workspace_id, "delete-me.txt", 42, "text/plain")
        .await
        .unwrap();
    let document_id = Uuid::parse_str(&document.id).unwrap();
    let task = ingestion::build_ingest_task(
        owner_user_id.to_string(),
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
    assert!(repo.ingestion_queue().enqueue_ingestion_task(&task).await.unwrap());

    assert_eq!(
        repo.documents().delete_document(&ctx, document_id).await.unwrap(),
        DocumentDeletionOutcome::Queued {
            task_inserted: true
        }
    );
    assert_eq!(
        repo.documents().delete_document(&ctx, document_id).await.unwrap(),
        DocumentDeletionOutcome::AlreadyDeleting {
            task_inserted: false
        }
    );
    assert_eq!(
        repo.documents().get_document_status(&ctx, document_id).await.unwrap(),
        Some(DocumentStatus::Deleting)
    );
    assert_eq!(
        repo.chunks().count_document_cleanup_tasks_for_document(&ctx, document_id)
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
        repo.list_documents(&ctx, Some(workspace_id), Some(document_id))
            .await
            .unwrap()
            .is_empty()
    );
}

