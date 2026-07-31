use super::support::*;

#[tokio::test]
async fn replace_table_evidence_chunks_idempotent_and_hydratable_when_database_available() {
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
        .bootstrap()
        .create_workspace(&ctx, "table evidence test notebook", "table evidence test")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&notebook.id).unwrap();
    let document = repo
        .bootstrap()
        .create_document(&ctx, workspace_id, "tables.md", 42, "text/markdown")
        .await
        .unwrap();
    let document_id = Uuid::parse_str(&document.id).unwrap();

    let c1 = TableEvidenceChunkRow {
        chunk_id: Uuid::new_v4(),
        table: "t0".into(),
        start_line: 10,
        n_rows: 370,
        md: "| 编号 | 活动 |\n| --- | --- |\n| 1 | x |\n".into(),
    };
    let c2 = TableEvidenceChunkRow {
        chunk_id: Uuid::new_v4(),
        table: "t1".into(),
        start_line: 500,
        n_rows: 12,
        md: "| a |\n| --- |\n| 1 |\n".into(),
    };
    // 首次装载 2 条
    let n = repo
        .assets()
        .replace_table_evidence_chunks(&ctx, document_id, &[c1.clone(), c2.clone()])
        .await
        .unwrap();
    assert_eq!(n, 2);
    // 水合可见（get_chunks_by_ids 接受 'table_evidence'，2b 已开）
    let got = repo
        .assets()
        .get_chunks_by_ids(&ctx, &[c1.chunk_id, c2.chunk_id])
        .await
        .unwrap();
    assert_eq!(got.len(), 2);
    assert!(got[&c1.chunk_id].content.contains("| 编号 | 活动 |"));
    // 幂等重载：替换为 1 条 → 旧的 t1 消失
    let c3 = TableEvidenceChunkRow { chunk_id: Uuid::new_v4(), ..c1.clone() };
    let n = repo
        .assets()
        .replace_table_evidence_chunks(&ctx, document_id, &[c3.clone()])
        .await
        .unwrap();
    assert_eq!(n, 1);
    let got = repo
        .assets()
        .get_chunks_by_ids(&ctx, &[c1.chunk_id, c2.chunk_id, c3.chunk_id])
        .await
        .unwrap();
    assert_eq!(got.len(), 1);
    assert!(got.contains_key(&c3.chunk_id));
    // document 不存在 → 该行跳过（INSERT ... SELECT ... FROM documents 同语义）
    let n = repo
        .assets()
        .replace_table_evidence_chunks(&ctx, Uuid::new_v4(), &[c1.clone()])
        .await
        .unwrap();
    assert_eq!(n, 0);
}
