use super::support::*;

#[tokio::test]
async fn generic_status_update_rejects_deleting_and_deleted_when_database_available() {
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
        .bootstrap().create_notebook(&ctx, "status guard test notebook", "status guard test")
        .await
        .unwrap();
    let document = repo
        .bootstrap().create_document(
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
            .documents().update_document(
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
            .documents().set_document_status(&ctx, document_id, DocumentStatus::Deleted)
            .await
            .unwrap()
    );
    assert_eq!(
        repo.documents().get_document_status(&ctx, document_id).await.unwrap(),
        Some(DocumentStatus::Pending)
    );
    assert_eq!(
        repo.chunks().count_document_cleanup_tasks_for_document(&ctx, document_id)
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
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository { pool: __bootstrap.pool.clone() };
    repo.bootstrap().migrate().await.unwrap();

    let org_id = OrgId::from(Uuid::new_v4());
    let user_id = Uuid::new_v4();
    let ctx = AuthContext::new(org_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(user_id));

    let notebook = repo
        .bootstrap().create_notebook(&ctx, "summary test notebook", "summary test")
        .await
        .unwrap();
    let document = repo
        .bootstrap().create_document(
            &ctx,
            Uuid::parse_str(&notebook.id).unwrap(),
            "summary-test.txt",
            42,
            "text/plain",
        )
        .await
        .unwrap();
    let document_id = Uuid::parse_str(&document.id).unwrap();

    repo.bootstrap().store_document_body(&ctx, document_id, "First line. Second line. Third line.")
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
    repo.documents().update_document_summary(&ctx, document_id, &summary_output, None, None)
        .await
        .unwrap();

    let preview = repo
        .chunks().get_parsed_preview(&ctx, document_id, 0, 10)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(preview.summary.as_deref(), Some("LLM upgraded summary"));
}

