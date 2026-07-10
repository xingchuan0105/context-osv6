    async fn billing_overview(&self, auth: &AuthContext) -> Result<AdminBillingOverview, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let row = sqlx::query(
            r#"
            select
              count(*) filter (where status = 'active') as active_subscriptions,
              count(*) filter (where status = 'past_due') as past_due_subscriptions,
              count(*) filter (where status = 'unpaid') as unpaid_subscriptions,
              count(*) filter (where status = 'canceled') as canceled_subscriptions
            from subscriptions
            "#,
        )
        .fetch_one(tx.as_mut())
        .await
        .map_err(db_err)?;
        tx.commit()
            .await
            .map_err(db_err)?;
        Ok(AdminBillingOverview {
            active_subscriptions: row.try_get("active_subscriptions").unwrap_or(0),
            past_due_subscriptions: row.try_get("past_due_subscriptions").unwrap_or(0),
            unpaid_subscriptions: row.try_get("unpaid_subscriptions").unwrap_or(0),
            canceled_subscriptions: row.try_get("canceled_subscriptions").unwrap_or(0),
        })
    }

    async fn get_usage(
        &self,
        auth: &AuthContext,
        owner_user_id: UserId,
        period: &str,
    ) -> Result<AdminUsageStats, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let since = admin_usage_period_start(period);
        let row = sqlx::query(
            r#"
            select
              (select count(*) from chat_messages m where m.owner_user_id = $1 and m.role = 'user' and m.created_at >= $2) as query_count,
              (select count(*) from documents d where d.owner_user_id = $1 and d.created_at >= $2) as document_count,
              (select count(*) from chunks c where c.owner_user_id = $1 and c.created_at >= $2) as chunk_count,
              (select coalesce(sum(d.file_size), 0)::bigint from documents d where d.owner_user_id = $1) as storage_bytes
            "#,
        )
        .bind(owner_user_id.into_uuid())
        .bind(since)
        .fetch_one(tx.as_mut())
        .await
        .map_err(db_err)?;
        tx.commit()
            .await
            .map_err(db_err)?;
        Ok(AdminUsageStats {
            owner_user_id,
            period: period.to_string(),
            query_count: row
                .try_get("query_count")
                .map_err(db_err)?,
            document_count: row
                .try_get("document_count")
                .map_err(db_err)?,
            chunk_count: row
                .try_get("chunk_count")
                .map_err(db_err)?,
            storage_bytes: row
                .try_get("storage_bytes")
                .map_err(db_err)?,
        })
    }

