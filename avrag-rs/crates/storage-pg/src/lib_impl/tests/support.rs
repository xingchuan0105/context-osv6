//! Shared helpers for storage-pg lib_impl tests.
pub(super) use super::super::*;
pub(super) use std::env;

pub(super) async fn insert_test_document_block(
    repo: &PgAppRepository,
    org_id: Uuid,
    workspace_id: Uuid,
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
            org_id, workspace_id, document_id, block_id, page, block_type, modality,
            text, parser_backend
        ) values ($1, $2, $3, $4, 1, 'paragraph', 'text', 'block text', 'test')
        "#,
    )
    .bind(org_id)
    .bind(workspace_id)
    .bind(document_id)
    .bind(block_id)
    .execute(tx.as_mut())
    .await
    .unwrap();
    tx.commit().await.unwrap();
}

pub(super) async fn count_document_assets_for_org(
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
        "select count(*)::bigint as c from document_assets where org_id = $1 and document_id = $2",
    )
    .bind(org_id)
    .bind(document_id)
    .fetch_one(tx.as_mut())
    .await
    .unwrap();
    tx.commit().await.unwrap();
    row.try_get("c").unwrap()
}

pub(super) async fn count_document_blocks_for_org(
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
