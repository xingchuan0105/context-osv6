use super::support::*;

#[tokio::test]
async fn cleanup_targets_reject_active_document_when_database_available() {
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
        .bootstrap().create_workspace(
            &ctx,
            "active cleanup guard notebook",
            "active cleanup guard",
        )
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&notebook.id).unwrap();
    let document = repo
        .bootstrap().create_document(&ctx, workspace_id, "active.txt", 42, "text/plain")
        .await
        .unwrap();
    let document_id = Uuid::parse_str(&document.id).unwrap();
    let mut tx = repo.raw().begin().await.unwrap();
    sqlx::query("select set_config('app.current_user', $1, true)")
        .bind(owner_user_id.into_uuid().to_string())
        .execute(tx.as_mut())
        .await
        .unwrap();
    sqlx::query(
        r#"
        insert into document_assets (
            asset_id, owner_user_id, workspace_id, document_id, page, asset_kind,
            storage_path, mime_type, parser_backend
        ) values ($1, $2, $3, $4, 1, 'image', 'must/not/delete.png', 'image/png', 'test')
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(owner_user_id.into_uuid())
    .bind(workspace_id)
    .bind(document_id)
    .execute(tx.as_mut())
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let payload = serde_json::json!({"object_path": "must/not/delete.txt"});
    assert!(
        repo.chunks().get_document_cleanup_targets(&ctx, document_id, &payload)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        !repo
            .chunks().cleanup_document_derived_rows(&ctx, document_id)
            .await
            .unwrap()
    );
    let remaining = count_document_assets_for_org(&repo, owner_user_id.into_uuid(), document_id).await;
    assert_eq!(remaining, 1);
}

