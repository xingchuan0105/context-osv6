    async fn get_share_analytics(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<Vec<ShareAnalyticsEntry>, AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_org(tx.as_mut(), &auth.org_id().to_string()).await?;
        let rows = sqlx::query(
            r#"
            select
                st.token,
                st.access_level,
                count(sal.id) as total_views,
                max(sal.created_at) as last_accessed_at,
                st.created_at
            from share_tokens st
            left join share_access_logs sal on sal.share_token = st.token
            where st.org_id = $1 and st.workspace_id = $2
            group by st.token, st.access_level, st.created_at
            order by total_views desc, max(sal.created_at) desc nulls last
            "#,
        )
        .bind(auth.org_id().into_uuid())
        .bind(workspace_id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|row| ShareAnalyticsEntry {
                token: row.try_get("token").unwrap_or_default(),
                access_level: row.try_get("access_level").unwrap_or_default(),
                total_views: row.try_get::<i64, _>("total_views").unwrap_or_default(),
                last_accessed_at: row
                    .try_get::<Option<DateTime<Utc>>, _>("last_accessed_at")
                    .ok()
                    .flatten()
                    .map(|dt| dt.timestamp()),
                created_at: row
                    .try_get::<Option<DateTime<Utc>>, _>("created_at")
                    .ok()
                    .flatten()
                    .map(|dt| dt.to_rfc3339()),
            })
            .collect())
    }

    async fn get_share_access_logs(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        limit: usize,
    ) -> Result<Vec<ShareAccessLogEntry>, AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_org(tx.as_mut(), &auth.org_id().to_string()).await?;
        let rows = sqlx::query(
            r#"
            select sal.id, sal.workspace_id, sal.share_token, sal.action, sal.created_at
            from share_access_logs sal
            join share_tokens st on st.token = sal.share_token
            where st.org_id = $1 and st.workspace_id = $2
            order by sal.created_at desc
            limit $3
            "#,
        )
        .bind(auth.org_id().into_uuid())
        .bind(workspace_id)
        .bind(limit as i64)
        .fetch_all(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|row| ShareAccessLogEntry {
                id: row
                    .try_get::<Uuid, _>("id")
                    .map(|u| u.to_string())
                    .unwrap_or_default(),
                workspace_id: row
                    .try_get::<Uuid, _>("workspace_id")
                    .map(|u| u.to_string())
                    .unwrap_or_default(),
                share_token: row.try_get("share_token").unwrap_or_default(),
                action: row.try_get("action").unwrap_or_default(),
                accessed_at: row
                    .try_get::<DateTime<Utc>, _>("created_at")
                    .map(|dt| dt.timestamp())
                    .unwrap_or_default(),
            })
            .collect())
    }
    async fn record_share_product_event(
        &self,
        event: analytics::ProductEvent,
    ) -> Result<(), AppError> {
        let analytics = analytics::AnalyticsService::new(self.repo.raw().clone());
        analytics
            .record_product_event(&event)
            .await
            .map_err(|error| AppError::internal(error.to_string()))
    }
