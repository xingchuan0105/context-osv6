use super::support::*;

#[tokio::test]
async fn list_body_chunk_md_line_ranges_filters_and_parses_when_database_available() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository { pool: __bootstrap.pool.clone() };
    repo.bootstrap().migrate().await.unwrap();

    let owner_uuid = Uuid::new_v4();
    let owner_user_id = UserId::from(owner_uuid);
    let user_id = Uuid::new_v4();
    let ctx = AuthContext::new(owner_user_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(user_id));
    let notebook = repo
        .bootstrap()
        .create_workspace(&ctx, "md line ranges test notebook", "md line ranges test")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&notebook.id).unwrap();
    let document = repo
        .bootstrap()
        .create_document(&ctx, workspace_id, "lines.md", 42, "text/markdown")
        .await
        .unwrap();
    let document_id = Uuid::parse_str(&document.id).unwrap();

    // 3 个 body chunk：两个带 md 行区间（ingestion 实际形状为 BTreeMap<String,String>
    // 序列化 → 字符串值），一个缺键（老数据/非 markitdown 路径形状）。
    let chunks = vec![
        StoreDocumentChunkParams {
            parse_run_id: None,
            page: Some(1),
            content: "c0".into(),
            metadata: serde_json::json!({
                "block_metadata": {"md_line_start": "10", "md_line_end": "19"}
            }),
        },
        StoreDocumentChunkParams {
            parse_run_id: None,
            page: Some(1),
            content: "c1".into(),
            metadata: serde_json::json!({
                "block_metadata": {"md_line_start": "2", "md_line_end": "9"}
            }),
        },
        StoreDocumentChunkParams {
            parse_run_id: None,
            page: Some(1),
            content: "c2 no lines".into(),
            metadata: serde_json::json!({"kind": "paragraph"}),
        },
    ];
    let stored = __bootstrap
        .store_document_body_chunks(&ctx, document_id, None, "c0\nc1", &chunks)
        .await
        .unwrap();
    assert_eq!(stored.len(), 3);

    let rows = repo
        .assets()
        .list_body_chunk_md_line_ranges(&ctx, document_id)
        .await
        .unwrap();
    assert_eq!(rows.len(), 2, "缺 md 行区间键的 body chunk 须被排除");
    // ORDER BY md_line_start：2 在前、10 在后；字符串值经 ::bigint 解析。
    assert_eq!(rows[0].md_line_start, 2);
    assert_eq!(rows[0].md_line_end, 9);
    assert_eq!(rows[1].md_line_start, 10);
    assert_eq!(rows[1].md_line_end, 19);
    let ids: Vec<String> = rows.iter().map(|r| r.chunk_id.to_string()).collect();
    assert!(ids.contains(&stored[0].chunk_id));
    assert!(ids.contains(&stored[1].chunk_id));

    // table_evidence 即使带 block_metadata 行号也不进列表（chunk_type='body' 过滤）。
    {
        let mut tx = repo.raw().begin().await.unwrap();
        sqlx::query("select set_config('app.current_user', $1, true)")
            .bind(owner_uuid.to_string())
            .execute(tx.as_mut())
            .await
            .unwrap();
        sqlx::query(
            "insert into chunks (owner_user_id, document_id, chunk_type, content, metadata)
             values ($1, $2, 'table_evidence', 'evidence', $3)",
        )
        .bind(owner_uuid)
        .bind(document_id)
        .bind(serde_json::json!({
            "block_metadata": {"md_line_start": "0", "md_line_end": "1"}
        }))
        .execute(tx.as_mut())
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }
    let rows = repo
        .assets()
        .list_body_chunk_md_line_ranges(&ctx, document_id)
        .await
        .unwrap();
    assert_eq!(rows.len(), 2, "table_evidence 须被排除");

    // 未知 document → 空（非错误）。
    let rows = repo
        .assets()
        .list_body_chunk_md_line_ranges(&ctx, Uuid::new_v4())
        .await
        .unwrap();
    assert!(rows.is_empty());
}
