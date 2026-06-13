    async fn ensure_admin_access(&self, auth: &AuthContext) -> Result<(), AppError> {
        let tx = self.begin_admin_tx(auth).await?;
        tx.commit().await.map_err(db_err)
    }

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

    async fn rag_health(&self, auth: &AuthContext) -> Result<AdminRagHealthStatus, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let response = AdminRagHealthStatus {
            failed_documents: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from documents where status in ('failed','Failed')",
            )
            .await,
            queued_tasks: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from ingestion_tasks where status = 'queued'",
            )
            .await,
            processing_tasks: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from ingestion_tasks where status in ('claimed','processing')",
            )
            .await,
            dead_letter_tasks: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from ingestion_tasks where status = 'dead_letter' or dead_lettered_at is not null",
            )
            .await,
            recent_guard_events: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from audit_log where action like '%guard%' and created_at >= now() - interval '24 hours'",
            )
            .await,
        };
        tx.commit()
            .await
            .map_err(db_err)?;
        Ok(response)
    }

    async fn worker_status(&self, auth: &AuthContext) -> Result<AdminWorkerStatus, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let response = AdminWorkerStatus {
            runtime_mode: "milvus",
            queued_tasks: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from ingestion_tasks where status = 'queued'",
            )
            .await,
            processing_tasks: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from ingestion_tasks where status in ('claimed','processing')",
            )
            .await,
            dead_letter_tasks: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from ingestion_tasks where status = 'dead_letter' or dead_lettered_at is not null",
            )
            .await,
            failed_documents: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from documents where status in ('failed','Failed')",
            )
            .await,
        };
        tx.commit()
            .await
            .map_err(db_err)?;
        Ok(response)
    }

    async fn degradation_status(
        &self,
        auth: &AuthContext,
    ) -> Result<AdminDegradationStatus, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let response = AdminDegradationStatus {
            failed_documents: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from documents where status in ('failed','Failed')",
            )
            .await,
            recent_guard_events: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from audit_log where action like '%guard%' and created_at >= now() - interval '24 hours'",
            )
            .await,
            share_access_events: Self::scalar_count(
                tx.as_mut(),
                "select count(*) from share_access_logs where created_at >= now() - interval '24 hours'",
            )
            .await,
        };
        tx.commit()
            .await
            .map_err(db_err)?;
        Ok(response)
    }
