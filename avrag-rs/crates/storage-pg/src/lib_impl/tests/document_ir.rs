use super::support::*;

#[tokio::test]
async fn document_ir_projection_deletes_are_tenant_scoped_when_database_available() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository { pool: __bootstrap.pool.clone() };
    repo.bootstrap().migrate().await.unwrap();

    let org_id = OrgId::from(Uuid::new_v4());
    let owner_org_uuid = org_id.into_uuid();
    let other_org_uuid = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let ctx = AuthContext::new(org_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(user_id));
    let notebook = repo
        .bootstrap().create_workspace(&ctx, "ir tenant scope notebook", "ir tenant scope")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&notebook.id).unwrap();
    let document = repo
        .bootstrap().create_document(&ctx, workspace_id, "ir-tenant-scope.txt", 42, "text/plain")
        .await
        .unwrap();
    let document_id = Uuid::parse_str(&document.id).unwrap();
    let other_workspace_id = Uuid::new_v4();

    insert_test_document_block(
        &repo,
        owner_org_uuid,
        workspace_id,
        document_id,
        "owner-clear-block",
    )
    .await;
    insert_test_document_block(
        &repo,
        other_org_uuid,
        other_workspace_id,
        document_id,
        "other-clear-block",
    )
    .await;

    repo.documents().clear_document_ir_projection(&ctx, document_id)
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
        workspace_id,
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

    repo.documents().replace_document_blocks(&ctx, workspace_id, document_id, &[replacement])
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

