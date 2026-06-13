    async fn list_feature_flags(
        &self,
        auth: &AuthContext,
    ) -> Result<Vec<AdminFeatureFlagEntry>, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let rows = sqlx::query(
            r#"
            select f.key, f.enabled, f.source, extract(epoch from f.updated_at)::bigint as updated_at,
              exists(select 1 from feature_flag_change_requests r where r.flag_key = f.key and r.status = 'pending') as has_pending_request
            from feature_flags f
            order by f.key asc
            "#,
        )
        .fetch_all(tx.as_mut())
        .await
        .map_err(db_err)?;
        tx.commit()
            .await
            .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|row| AdminFeatureFlagEntry {
                key: row.try_get("key").unwrap_or_default(),
                category: "runtime".to_string(),
                description: String::new(),
                enabled: row.try_get("enabled").unwrap_or(false),
                effective_enabled: row.try_get("enabled").unwrap_or(false),
                config_ready: true,
                requires_config: false,
                source: row
                    .try_get("source")
                    .unwrap_or_else(|_| "admin_panel".to_string()),
                updated_at: row.try_get("updated_at").ok(),
                has_pending_request: row.try_get("has_pending_request").unwrap_or(false),
            })
            .collect())
    }

    async fn list_feature_flag_change_requests(
        &self,
        auth: &AuthContext,
        status: Option<&str>,
    ) -> Result<Vec<AdminFeatureFlagChangeRequest>, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let rows = if let Some(status) = status.filter(|value| !value.trim().is_empty()) {
            sqlx::query("select *, extract(epoch from created_at)::bigint as created_epoch, extract(epoch from reviewed_at)::bigint as reviewed_epoch, extract(epoch from executed_at)::bigint as executed_epoch from feature_flag_change_requests where status = $1 order by created_at desc")
                .bind(status)
                .fetch_all(tx.as_mut())
                .await
        } else {
            sqlx::query("select *, extract(epoch from created_at)::bigint as created_epoch, extract(epoch from reviewed_at)::bigint as reviewed_epoch, extract(epoch from executed_at)::bigint as executed_epoch from feature_flag_change_requests order by created_at desc")
                .fetch_all(tx.as_mut())
                .await
        }
        .map_err(db_err)?;
        tx.commit()
            .await
            .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(Self::map_feature_flag_change_request)
            .collect())
    }

    async fn create_feature_flag_change_request(
        &self,
        auth: &AuthContext,
        key: &str,
        enabled: bool,
        reason: &str,
    ) -> Result<AdminFeatureFlagChangeRequest, AppError> {
        let actor_id = auth.actor_id().ok_or_else(|| {
            AppError::unauthorized("admin action requires an authenticated user")
        })?;
        let mut tx = self.begin_admin_tx(auth).await?;
        sqlx::query(
            "insert into feature_flags (key, enabled) values ($1, false) on conflict (key) do nothing",
        )
        .bind(key)
        .execute(tx.as_mut())
        .await
        .map_err(db_err)?;
        let current_enabled =
            sqlx::query_scalar::<_, bool>("select enabled from feature_flags where key = $1")
                .bind(key)
                .fetch_one(tx.as_mut())
                .await
                .unwrap_or(false);
        let id = Uuid::new_v4().to_string();
        let row = sqlx::query("insert into feature_flag_change_requests (id, flag_key, current_enabled, requested_enabled, reason, status, requested_by) values ($1, $2, $3, $4, $5, 'pending', $6) returning *, extract(epoch from created_at)::bigint as created_epoch, extract(epoch from reviewed_at)::bigint as reviewed_epoch, extract(epoch from executed_at)::bigint as executed_epoch")
            .bind(&id)
            .bind(key)
            .bind(current_enabled)
            .bind(enabled)
            .bind(reason)
            .bind(actor_id.into_uuid())
            .fetch_one(tx.as_mut())
            .await
            .map_err(db_err)?;
        let response = Self::map_feature_flag_change_request(row);
        tx.commit()
            .await
            .map_err(db_err)?;
        Ok(response)
    }

    async fn review_feature_flag_change_request(
        &self,
        auth: &AuthContext,
        request_id: &str,
        approved: bool,
        review_note: Option<&str>,
    ) -> Result<AdminFeatureFlagChangeRequest, AppError> {
        let actor_id = auth.actor_id().ok_or_else(|| {
            AppError::unauthorized("admin action requires an authenticated user")
        })?;
        let mut tx = self.begin_admin_tx(auth).await?;
        let status = if approved { "approved" } else { "rejected" };
        let row = sqlx::query("update feature_flag_change_requests set status = $2, reviewed_by = $3, review_note = $4, reviewed_at = now(), executed_at = case when $2 = 'approved' then now() else executed_at end where id = $1 returning *, extract(epoch from created_at)::bigint as created_epoch, extract(epoch from reviewed_at)::bigint as reviewed_epoch, extract(epoch from executed_at)::bigint as executed_epoch")
            .bind(request_id)
            .bind(status)
            .bind(actor_id.into_uuid())
            .bind(review_note)
            .fetch_one(tx.as_mut())
            .await
            .map_err(db_err)?;
        let response = Self::map_feature_flag_change_request(row);
        tx.commit()
            .await
            .map_err(db_err)?;
        Ok(response)
    }
