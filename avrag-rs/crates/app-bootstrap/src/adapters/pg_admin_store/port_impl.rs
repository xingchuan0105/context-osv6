#[async_trait]
impl AdminStorePort for PgAdminStoreAdapter {
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
    async fn get_user_profile(
        &self,
        auth: &AuthContext,
        user_id: Uuid,
    ) -> Result<Option<UserProfileRow>, AppError> {
        self.repo
            .get_user_profile(auth, user_id)
            .await
            .map_err(map_pg_error)
            .map(|profile| profile.map(user_profile_row))
    }

    async fn upsert_user_profile(
        &self,
        auth: &AuthContext,
        profile: &UserProfileRow,
    ) -> Result<(), AppError> {
        self.repo
            .upsert_user_profile(auth, &user_profile_row_to_pg(profile))
            .await
            .map_err(map_pg_error)
    }

    async fn list_api_keys(
        &self,
        auth: &AuthContext,
        notebook_id: Option<Uuid>,
    ) -> Result<Vec<ApiKeyRow>, AppError> {
        self.repo
            .list_api_keys(auth, notebook_id)
            .await
            .map_err(map_pg_error)
    }

    async fn create_api_key(
        &self,
        auth: &AuthContext,
        notebook_id: Option<Uuid>,
        name: &str,
        permissions: &[String],
        rate_limit_rpm: i32,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<CreateApiKeyResponse, AppError> {
        let (api_key, plaintext_key) = self
            .repo
            .create_api_key(
                auth,
                notebook_id,
                name,
                permissions,
                rate_limit_rpm.max(0) as u32,
                expires_at,
            )
            .await
            .map_err(map_pg_error)?;
        Ok(CreateApiKeyResponse {
            api_key,
            plaintext_key,
        })
    }

    async fn revoke_api_key(
        &self,
        auth: &AuthContext,
        notebook_id: Option<Uuid>,
        key_id: Uuid,
    ) -> Result<bool, AppError> {
        self.repo
            .revoke_api_key(auth, notebook_id, key_id)
            .await
            .map_err(map_pg_error)
    }

    async fn list_notifications(
        &self,
        auth: &AuthContext,
        user_id: Uuid,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<NotificationRow>, AppError> {
        self.repo
            .list_notifications(auth, user_id, limit, offset)
            .await
            .map_err(map_pg_error)
    }

    async fn mark_notification_read(
        &self,
        auth: &AuthContext,
        user_id: Uuid,
        notification_id: Uuid,
    ) -> Result<bool, AppError> {
        self.repo
            .mark_notification_read(auth, user_id, notification_id)
            .await
            .map_err(map_pg_error)
    }
    async fn list_orgs(
        &self,
        auth: &AuthContext,
        page: usize,
        per_page: usize,
    ) -> Result<Vec<AdminOrgInfo>, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let page = page.max(1);
        let per_page = admin_clamp_org_list_per_page(per_page);
        let offset = ((page - 1) * per_page) as i64;
        let rows = sqlx::query(
            r#"
            select
              o.id,
              o.name,
              o.created_at,
              o.blocked,
              count(distinct u.id) as user_count,
              count(distinct d.id) as document_count,
              count(distinct m.id) filter (where m.role = 'user') as query_count
            from organizations o
            left join users u on u.org_id = o.id
            left join documents d on d.org_id = o.id
            left join chat_messages m on m.org_id = o.id
            group by o.id, o.name, o.created_at, o.blocked
            order by o.created_at desc
            limit $1 offset $2
            "#,
        )
        .bind(per_page as i64)
        .bind(offset)
        .fetch_all(tx.as_mut())
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        rows.into_iter().map(Self::map_org_info).collect()
    }

    async fn get_org(&self, auth: &AuthContext, org_id: OrgId) -> Result<AdminOrgInfo, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let row = sqlx::query(
            r#"
            select
              o.id,
              o.name,
              o.created_at,
              o.blocked,
              (select count(*) from users u where u.org_id = o.id) as user_count,
              (select count(*) from documents d where d.org_id = o.id) as document_count,
              (select count(*) from chat_messages m where m.org_id = o.id and m.role = 'user') as query_count
            from organizations o
            where o.id = $1
            "#,
        )
        .bind(org_id.into_uuid())
        .fetch_optional(tx.as_mut())
        .await
        .map_err(db_err)?;
        tx.commit()
            .await
            .map_err(db_err)?;
        row.map(Self::map_org_info)
            .transpose()?
            .ok_or_else(|| AppError::not_found("org_not_found", "Organization not found"))
    }

    async fn list_users(
        &self,
        auth: &AuthContext,
        org_id: OrgId,
    ) -> Result<Vec<AdminUserInfo>, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let rows = sqlx::query(
            r#"
            select id, email, org_id, role, created_at
            from users
            where org_id = $1
            order by created_at asc
            "#,
        )
        .bind(org_id.into_uuid())
        .fetch_all(tx.as_mut())
        .await
        .map_err(db_err)?;
        tx.commit()
            .await
            .map_err(db_err)?;
        rows.into_iter().map(Self::map_user_info).collect()
    }

    async fn delete_user(
        &self,
        auth: &AuthContext,
        org_id: OrgId,
        user_id: Uuid,
    ) -> Result<(), AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let exists: bool = sqlx::query_scalar(
            "select exists(select 1 from users where id = $1 and org_id = $2)",
        )
        .bind(user_id)
        .bind(org_id.into_uuid())
        .fetch_one(tx.as_mut())
        .await
        .map_err(db_err)?;
        if !exists {
            return Err(AppError::not_found(
                "user_not_found",
                "User not found in this organization",
            ));
        }
        let deleted: i64 = sqlx::query_scalar("select delete_user_cascade($1)")
            .bind(user_id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(db_err)?;
        tx.commit()
            .await
            .map_err(db_err)?;
        if deleted > 0 {
            Ok(())
        } else {
            Err(AppError::not_found(
                "user_not_found",
                "User not found in this organization",
            ))
        }
    }

    async fn get_usage(
        &self,
        auth: &AuthContext,
        org_id: OrgId,
        period: &str,
    ) -> Result<AdminUsageStats, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let since = admin_usage_period_start(period);
        let row = sqlx::query(
            r#"
            select
              (select count(*) from chat_messages m where m.org_id = $1 and m.role = 'user' and m.created_at >= $2) as query_count,
              (select count(*) from documents d where d.org_id = $1 and d.created_at >= $2) as document_count,
              (select count(*) from chunks c where c.org_id = $1 and c.created_at >= $2) as chunk_count,
              (select coalesce(sum(d.file_size), 0)::bigint from documents d where d.org_id = $1) as storage_bytes
            "#,
        )
        .bind(org_id.into_uuid())
        .bind(since)
        .fetch_one(tx.as_mut())
        .await
        .map_err(db_err)?;
        tx.commit()
            .await
            .map_err(db_err)?;
        Ok(AdminUsageStats {
            org_id,
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

    async fn set_org_blocked(
        &self,
        auth: &AuthContext,
        org_id: OrgId,
        blocked: bool,
    ) -> Result<(), AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let result = sqlx::query("update organizations set blocked = $2 where id = $1")
            .bind(org_id.into_uuid())
            .bind(blocked)
            .execute(tx.as_mut())
            .await
            .map_err(db_err)?;
        if result.rows_affected() == 0 {
            return Err(AppError::not_found(
                "org_not_found",
                "Organization not found",
            ));
        }
        tx.commit().await.map_err(db_err)?;
        Ok(())
    }
    async fn list_audit_logs(
        &self,
        auth: &AuthContext,
        query: &AdminAuditLogQuery,
    ) -> Result<AdminAuditLogPage, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let total = Self::audit_log_total(tx.as_mut(), query).await?;
        let rows = Self::audit_log_rows(tx.as_mut(), query).await?;
        tx.commit()
            .await
            .map_err(db_err)?;
        Ok(AdminAuditLogPage {
            items: rows
                .into_iter()
                .map(Self::map_audit_log_entry)
                .collect::<Result<Vec<_>, _>>()?,
            total,
            page: query.page.max(1),
            per_page: admin_clamp_audit_per_page(query.per_page),
        })
    }

    async fn export_audit_logs_csv(
        &self,
        auth: &AuthContext,
        query: &AdminAuditLogQuery,
    ) -> Result<String, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let export_query = AdminAuditLogQuery {
            query: query.query.clone(),
            action: query.action.clone(),
            resource_type: query.resource_type.clone(),
            actor: query.actor.clone(),
            window: query.window.clone(),
            page: 1,
            per_page: 5_000,
        };
        let rows = Self::audit_log_rows(tx.as_mut(), &export_query).await?;
        tx.commit()
            .await
            .map_err(db_err)?;
        Ok(admin_audit_logs_to_csv(
            &rows
                .into_iter()
                .map(Self::map_audit_log_entry)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }
}
