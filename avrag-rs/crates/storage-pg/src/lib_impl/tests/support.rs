//! Shared helpers for storage-pg lib_impl tests.
pub(super) use super::super::*;
pub(super) use std::env;

pub(super) async fn insert_test_document_block(
    repo: &PgAppRepository,
    owner_user_id: Uuid,
    workspace_id: Uuid,
    document_id: Uuid,
    block_id: &str,
) {
    let mut tx = repo.raw().begin().await.unwrap();
    sqlx::query("select set_config('app.current_user', $1, true)")
        .bind(owner_user_id.to_string())
        .execute(tx.as_mut())
        .await
        .unwrap();
    sqlx::query(
        r#"
        insert into document_blocks (
            owner_user_id, workspace_id, document_id, block_id, page, block_type, modality,
            text, parser_backend
        ) values ($1, $2, $3, $4, 1, 'paragraph', 'text', 'block text', 'test')
        "#,
    )
    .bind(owner_user_id)
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
    owner_user_id: Uuid,
    document_id: Uuid,
) -> i64 {
    let mut tx = repo.raw().begin().await.unwrap();
    sqlx::query("select set_config('app.current_user', $1, true)")
        .bind(owner_user_id.to_string())
        .execute(tx.as_mut())
        .await
        .unwrap();
    let row = sqlx::query(
        "select count(*)::bigint as c from document_assets where owner_user_id = $1 and document_id = $2",
    )
    .bind(owner_user_id)
    .bind(document_id)
    .fetch_one(tx.as_mut())
    .await
    .unwrap();
    tx.commit().await.unwrap();
    row.try_get("c").unwrap()
}

pub(super) async fn count_document_blocks_for_org(
    repo: &PgAppRepository,
    owner_user_id: Uuid,
    document_id: Uuid,
) -> i64 {
    let mut tx = repo.raw().begin().await.unwrap();
    sqlx::query("select set_config('app.current_user', $1, true)")
        .bind(owner_user_id.to_string())
        .execute(tx.as_mut())
        .await
        .unwrap();
    let row = sqlx::query(
        "select count(*)::bigint as c from document_blocks where owner_user_id = $1 and document_id = $2",
    )
    .bind(owner_user_id)
    .bind(document_id)
    .fetch_one(tx.as_mut())
    .await
    .unwrap();
    tx.commit().await.unwrap();
    row.try_get("c").unwrap()
}
