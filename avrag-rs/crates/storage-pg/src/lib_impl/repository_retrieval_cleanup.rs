use super::*;
impl ChunkRepository {
    pub async fn count_document_cleanup_tasks_for_document(
        &self,
        context: &AuthContext,
        document_id: Uuid,
    ) -> Result<i64, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            select count(*)::bigint as task_count
            from document_cleanup_tasks
            where org_id = $1
              and document_id = $2
            "#,
        )
        .bind(context.org_id().into_uuid())
        .bind(document_id)
        .fetch_one(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(row.try_get("task_count")?)
    }

    pub async fn get_document_cleanup_targets(
        &self,
        context: &AuthContext,
        document_id: Uuid,
        task_payload: &serde_json::Value,
    ) -> Result<Option<DocumentCleanupTargets>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            select id, org_id, notebook_id, status, object_path
            from documents
            where id = $1
              and org_id = $2
              and status in ('deleting', 'deleted')
            "#,
        )
        .bind(document_id)
        .bind(context.org_id().into_uuid())
        .fetch_optional(tx.inner())
        .await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };
        let org_id: Uuid = row.try_get("org_id")?;
        let notebook_id: Uuid = row.try_get("notebook_id")?;

        let asset_rows = sqlx::query(
            r#"
            select storage_path
            from document_assets
            where org_id = $1
              and notebook_id = $2
              and document_id = $3
              and storage_path is not null
            order by created_at asc, asset_id asc
            "#,
        )
        .bind(org_id)
        .bind(notebook_id)
        .bind(document_id)
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;

        let object_path: Option<String> = row.try_get("object_path")?;
        let fallback_object_path = task_payload
            .get("object_path")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let status_text: String = row.try_get("status")?;
        Ok(Some(DocumentCleanupTargets {
            org_id,
            notebook_id,
            document_id: row.try_get("id")?,
            status: parse_document_status(&status_text),
            object_path: object_path.or(fallback_object_path),
            asset_storage_paths: asset_rows
                .into_iter()
                .filter_map(|row| {
                    row.try_get::<Option<String>, _>("storage_path")
                        .ok()
                        .flatten()
                })
                .collect(),
        }))
    }

    pub async fn cleanup_document_derived_rows(
        &self,
        context: &AuthContext,
        document_id: Uuid,
    ) -> Result<bool, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            select org_id, notebook_id
            from documents
            where id = $1
              and org_id = $2
              and status in ('deleting', 'deleted')
            for update
            "#,
        )
        .bind(document_id)
        .bind(context.org_id().into_uuid())
        .fetch_optional(tx.inner())
        .await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(false);
        };
        let org_id: Uuid = row.try_get("org_id")?;
        let notebook_id: Uuid = row.try_get("notebook_id")?;

        sqlx::query(
            "delete from document_multimodal_chunks where org_id = $1 and notebook_id = $2 and document_id = $3",
        )
        .bind(org_id)
        .bind(notebook_id)
        .bind(document_id)
        .execute(tx.inner())
        .await?;
        sqlx::query(
            "delete from document_assets where org_id = $1 and notebook_id = $2 and document_id = $3",
        )
        .bind(org_id)
        .bind(notebook_id)
        .bind(document_id)
        .execute(tx.inner())
        .await?;
        sqlx::query(
            "delete from document_blocks where org_id = $1 and notebook_id = $2 and document_id = $3",
        )
        .bind(org_id)
        .bind(notebook_id)
        .bind(document_id)
        .execute(tx.inner())
        .await?;
        sqlx::query("delete from chunks where org_id = $1 and document_id = $2")
            .bind(org_id)
            .bind(document_id)
            .execute(tx.inner())
            .await?;
        sqlx::query(
            "delete from document_parse_runs where org_id = $1 and notebook_id = $2 and document_id = $3",
        )
        .bind(org_id)
        .bind(notebook_id)
        .bind(document_id)
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn mark_document_deleted(
        &self,
        context: &AuthContext,
        document_id: Uuid,
    ) -> Result<bool, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let result = sqlx::query(
            r#"
            update documents
            set status = 'deleted',
                deleted_at = now(),
                deletion_error = null,
                updated_at = now()
            where id = $1
              and org_id = $2
              and status in ('deleting', 'deleted')
            "#,
        )
        .bind(document_id)
        .bind(context.org_id().into_uuid())
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn document_cleanup_task_lease_is_current(
        &self,
        task_id: Uuid,
        lock_token: Uuid,
    ) -> Result<bool, PgStorageError> {
        let mut tx = self.pool.raw().begin().await?;
        sqlx::query("select set_config('app.document_cleanup_worker', 'true', true)")
            .execute(tx.as_mut())
            .await?;
        let row = sqlx::query(
            r#"
            select exists(
                select 1
                from document_cleanup_tasks
                where task_id = $1
                  and lock_token = $2
                  and status = 'processing'
                  and dead_lettered_at is null
                  and completed_at is null
            ) as lease_current
            "#,
        )
        .bind(task_id)
        .bind(lock_token)
        .fetch_one(tx.as_mut())
        .await?;
        tx.commit().await?;
        Ok(row.try_get("lease_current")?)
    }

}
pub async fn insert_document_cleanup_task(
    tx: &mut PgConnection,
    org_id: Uuid,
    notebook_id: Uuid,
    document_id: Uuid,
    requested_by: Option<Uuid>,
    row: &PgRow,
) -> Result<bool, PgStorageError> {
    let file_name: String = row.try_get("file_name")?;
    let mime_type: Option<String> = row.try_get("mime_type")?;
    let file_size: i64 = row.try_get("file_size")?;
    let object_path: Option<String> = row.try_get("object_path")?;
    let status: String = row.try_get("status")?;
    let idempotency_key = format!("document-cleanup:{org_id}:{document_id}");
    let payload = json!({
        "org_id": org_id.to_string(),
        "notebook_id": notebook_id.to_string(),
        "document_id": document_id.to_string(),
        "file_name": file_name,
        "mime_type": mime_type.unwrap_or_default(),
        "file_size": u64::try_from(file_size).unwrap_or_default(),
        "object_path": object_path,
        "status_at_request": status,
    });

    let result = sqlx::query(
        r#"
        insert into document_cleanup_tasks (
            org_id, notebook_id, document_id, requested_by, idempotency_key, payload
        )
        values ($1, $2, $3, $4, $5, $6)
        on conflict (idempotency_key) do nothing
        "#,
    )
    .bind(org_id)
    .bind(notebook_id)
    .bind(document_id)
    .bind(requested_by)
    .bind(idempotency_key)
    .bind(payload)
    .execute(tx)
    .await?;

    Ok(result.rows_affected() > 0)
}
