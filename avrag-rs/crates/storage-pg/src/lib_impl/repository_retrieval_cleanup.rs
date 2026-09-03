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
            where owner_user_id = $1
              and document_id = $2
            "#,
        )
        .bind(context.user_id().into_uuid())
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
            select d.id, d.owner_user_id, wb.workspace_id, d.status, d.object_path
            from documents d
            left join lateral (
                select b.workspace_id
                from workspace_document_bindings b
                where b.artifact_id = d.id
                order by b.created_at asc, b.id asc
                limit 1
            ) wb on true
            where d.id = $1
              and d.owner_user_id = $2
              and d.status in ('deleting', 'deleted')
            "#,
        )
        .bind(document_id)
        .bind(context.user_id().into_uuid())
        .fetch_optional(tx.inner())
        .await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };
        let owner_user_id: Uuid = row.try_get("owner_user_id")?;
        let workspace_id: Option<Uuid> = row.try_get("workspace_id")?;

        let asset_rows = sqlx::query(
            r#"
            select storage_path
            from document_assets
            where owner_user_id = $1
              and document_id = $2
              and storage_path is not null
            order by created_at asc, asset_id asc
            "#,
        )
        .bind(owner_user_id)
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
            owner_user_id,
            workspace_id,
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
            select owner_user_id
            from documents
            where id = $1
              and owner_user_id = $2
              and status in ('deleting', 'deleted')
            for update
            "#,
        )
        .bind(document_id)
        .bind(context.user_id().into_uuid())
        .fetch_optional(tx.inner())
        .await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(false);
        };
        let owner_user_id: Uuid = row.try_get("owner_user_id")?;

        sqlx::query(
            "delete from document_multimodal_chunks where owner_user_id = $1 and document_id = $2",
        )
        .bind(owner_user_id)
        .bind(document_id)
        .execute(tx.inner())
        .await?;
        sqlx::query(
            "delete from document_assets where owner_user_id = $1 and document_id = $2",
        )
        .bind(owner_user_id)
        .bind(document_id)
        .execute(tx.inner())
        .await?;
        sqlx::query(
            "delete from document_blocks where owner_user_id = $1 and document_id = $2",
        )
        .bind(owner_user_id)
        .bind(document_id)
        .execute(tx.inner())
        .await?;
        sqlx::query("delete from chunks where owner_user_id = $1 and document_id = $2")
            .bind(owner_user_id)
            .bind(document_id)
            .execute(tx.inner())
            .await?;
        sqlx::query(
            "delete from document_parse_runs where owner_user_id = $1 and document_id = $2",
        )
        .bind(owner_user_id)
        .bind(document_id)
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    /// W2e citation tombstones: after a document's content stores are cleaned,
    /// prune its citations out of historical chat messages. Each stored citation
    /// keeps only irreducible facts (doc_id, name, page, ids); content-bearing
    /// keys are removed and `citation_status` marks the source as deleted.
    pub async fn prune_document_citations_to_tombstones(
        &self,
        context: &AuthContext,
        document_id: Uuid,
    ) -> Result<u64, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(tx.inner())
            .await?;
        let result = sqlx::query(
            r#"
            update chat_messages m
            set citations = sub.pruned
            from (
                select m2.id as message_id,
                       coalesce(
                           jsonb_agg(
                               case
                                   when c ->> 'doc_id' = $1::text
                                       then (c - 'content' - 'preview' - 'image_url' - 'asset_id')
                                            || '{"citation_status": "source_deleted"}'::jsonb
                                   else c
                               end
                               order by ord
                           ),
                           '[]'::jsonb
                       ) as pruned
                from chat_messages m2
                cross join lateral jsonb_array_elements(m2.citations) with ordinality as t(c, ord)
                where m2.citations @> jsonb_build_array(jsonb_build_object('doc_id', $1::text))
                group by m2.id
            ) sub
            where m.id = sub.message_id
            "#,
        )
        .bind(document_id.to_string())
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(result.rows_affected())
    }

    /// Shared orphan sweep for scope-owner deletion (review P1-4: one copy of
    /// the cleanup contract for conversation AND workspace deletion — zero-
    /// binding artifacts soft-delete + enter the async cleanup queue).
    pub async fn sweep_orphaned_artifacts(
        tx: &mut PgConnection,
        owner_user_id: Uuid,
        document_ids: &[Uuid],
        requested_by: Option<Uuid>,
        reason: &str,
    ) -> Result<usize, PgStorageError> {
        let mut swept = 0;
        for document_id in document_ids {
            let orphaned = sqlx::query_scalar::<_, bool>(
                r#"
                select not exists (
                    select 1 from conversation_document_bindings b where b.artifact_id = $1
                )
                and not exists (
                    select 1 from workspace_document_bindings b where b.artifact_id = $1
                )
                "#,
            )
            .bind(document_id)
            .fetch_one(&mut *tx)
            .await?;
            if !orphaned {
                continue;
            }
            sqlx::query(
                r#"
                update documents
                set status = 'deleting',
                    deletion_requested_at = coalesce(deletion_requested_at, now()),
                    deletion_error = null,
                    updated_at = now()
                where id = $1
                  and owner_user_id = $2
                  and status not in ('deleting', 'deleted')
                "#,
            )
            .bind(document_id)
            .bind(owner_user_id)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"
                insert into document_cleanup_tasks (
                    owner_user_id, workspace_id, document_id, requested_by, idempotency_key, payload
                )
                values ($1, null, $2, $3, $4, $5)
                on conflict (idempotency_key) do nothing
                "#,
            )
            .bind(owner_user_id)
            .bind(document_id)
            .bind(requested_by)
            .bind(format!("document-cleanup:{owner_user_id}:{document_id}"))
            .bind(serde_json::json!({
                "owner_user_id": owner_user_id.to_string(),
                "document_id": document_id.to_string(),
                "reason": reason,
            }))
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"
                update ingestion_tasks
                set status = 'dead_letter',
                    dead_lettered_at = coalesce(dead_lettered_at, now()),
                    last_failed_at = coalesce(last_failed_at, now()),
                    last_error = coalesce(last_error, 'document deletion requested'),
                    locked_at = null,
                    locked_by = null,
                    lock_token = null,
                    updated_at = now()
                where owner_user_id = $1
                  and document_id = $2
                  and status in ('queued', 'processing')
                  and dead_lettered_at is null
                "#,
            )
            .bind(owner_user_id)
            .bind(document_id)
            .execute(&mut *tx)
            .await?;
            swept += 1;
        }
        Ok(swept)
    }

    /// W2e review fix: `turn_evidence` segments must collapse with the
    /// citations — strip chunk/asset/parse identifiers and mark the source
    /// deleted, keeping the same irreducible facts as the citation tombstone.
    pub async fn prune_turn_evidence_to_tombstones(
        &self,
        context: &AuthContext,
        document_id: Uuid,
    ) -> Result<u64, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(tx.inner())
            .await?;
        let result = sqlx::query(
            r#"
            update chat_messages m
            set turn_metadata = jsonb_set(
                m.turn_metadata,
                '{turn_evidence,segments}',
                sub.pruned
            )
            from (
                select m2.id as message_id,
                       coalesce(
                           jsonb_agg(
                               case
                                   when seg ->> 'artifact_id' = $1::text
                                       then (seg - 'chunk_id' - 'asset_id' - 'parse_version')
                                            || '{"citation_status": "source_deleted"}'::jsonb
                                   else seg
                               end
                               order by ord
                           ),
                           '[]'::jsonb
                       ) as pruned
                from chat_messages m2
                cross join lateral jsonb_array_elements(
                    m2.turn_metadata -> 'turn_evidence' -> 'segments'
                ) with ordinality as t(seg, ord)
                where m2.turn_metadata ? 'turn_evidence'
                  and m2.turn_metadata -> 'turn_evidence' -> 'segments'
                        @> jsonb_build_array(jsonb_build_object('artifact_id', $1::text))
                group by m2.id
            ) sub
            where m.id = sub.message_id
            "#,
        )
        .bind(document_id.to_string())
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(result.rows_affected())
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
              and owner_user_id = $2
              and status in ('deleting', 'deleted')
            "#,
        )
        .bind(document_id)
        .bind(context.user_id().into_uuid())
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
    owner_user_id: Uuid,
    document_id: Uuid,
    requested_by: Option<Uuid>,
    row: &PgRow,
) -> Result<bool, PgStorageError> {
    let file_name: String = row.try_get("file_name")?;
    let mime_type: Option<String> = row.try_get("mime_type")?;
    let file_size: i64 = row.try_get("file_size")?;
    let object_path: Option<String> = row.try_get("object_path")?;
    let status: String = row.try_get("status")?;
    let idempotency_key = format!("document-cleanup:{owner_user_id}:{document_id}");
    let payload = json!({
        "owner_user_id": owner_user_id.to_string(),
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
            owner_user_id, workspace_id, document_id, requested_by, idempotency_key, payload
        )
        values ($1, null, $2, $3, $4, $5)
        on conflict (idempotency_key) do nothing
        "#,
    )
    .bind(owner_user_id)
    .bind(document_id)
    .bind(requested_by)
    .bind(idempotency_key)
    .bind(payload)
    .execute(tx)
    .await?;

    Ok(result.rows_affected() > 0)
}
