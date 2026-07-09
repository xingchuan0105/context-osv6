use super::*;
impl AuditRepository {
    pub async fn append_audit_record(&self, record: &AuditRecord) -> Result<(), PgStorageError> {
        let org_id = Uuid::parse_str(&record.org_id)
            .map_err(|_| PgStorageError::NotFound("invalid audit org id".to_string()))?;
        let context = AuthContext::new(OrgId::from(org_id), contracts::auth_runtime::SubjectKind::System);
        let mut tx = self.pool.begin(&context).await?;
        ensure_org_and_actor(tx.inner(), &context).await?;
        sqlx::query(
            r#"
            insert into audit_log (org_id, actor_id, action, resource_type, resource_id, payload, created_at)
            values ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(org_id)
        .bind(record.actor_id.as_deref().and_then(|value| Uuid::parse_str(value).ok()))
        .bind(record.action.as_str())
        .bind(&record.resource_type)
        .bind(&record.resource_id)
        .bind(&record.payload)
        .bind(parse_rfc3339(&record.created_at)?)
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Prune audit_log records older than the retention period.
    /// Returns the number of deleted rows.
    pub async fn prune_audit_log(
        &self,
        retention_days: i32,
    ) -> Result<u64, PgStorageError> {
        let result = sqlx::query(
            r#"
            delete from audit_log
            where created_at < now() - ($1::int * interval '1 day')
            "#,
        )
        .bind(retention_days)
        .execute(self.pool.raw())
        .await?;
        Ok(result.rows_affected())
    }
}
