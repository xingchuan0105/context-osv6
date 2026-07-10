impl PgAdminStoreAdapter {
    fn map_feature_flag_change_request(row: sqlx::postgres::PgRow) -> AdminFeatureFlagChangeRequest {
        AdminFeatureFlagChangeRequest {
            id: row.try_get("id").unwrap_or_default(),
            flag_key: row.try_get("flag_key").unwrap_or_default(),
            current_enabled: row.try_get("current_enabled").unwrap_or(false),
            requested_enabled: row.try_get("requested_enabled").unwrap_or(false),
            reason: row.try_get("reason").unwrap_or_default(),
            status: row.try_get("status").unwrap_or_default(),
            requested_by: row
                .try_get::<Uuid, _>("requested_by")
                .map(|value| value.to_string())
                .unwrap_or_default(),
            reviewed_by: row
                .try_get::<Option<Uuid>, _>("reviewed_by")
                .ok()
                .flatten()
                .map(|value| value.to_string()),
            review_note: row.try_get("review_note").ok(),
            created_at: row.try_get("created_epoch").unwrap_or(0),
            reviewed_at: row
                .try_get::<Option<i64>, _>("reviewed_epoch")
                .ok()
                .flatten(),
            executed_at: row
                .try_get::<Option<i64>, _>("executed_epoch")
                .ok()
                .flatten(),
        }
    }

    fn map_account_info(row: sqlx::postgres::PgRow) -> Result<AdminAccountInfo, AppError> {
        let id: Uuid = row.try_get("id").map_err(db_err)?;
        let created_at: DateTime<Utc> = row.try_get("created_at").map_err(db_err)?;
        Ok(AdminAccountInfo {
            id: UserId::from(id),
            name: row.try_get("name").map_err(db_err)?,
            created_at: created_at.timestamp(),
            blocked: row.try_get("blocked").map_err(db_err)?,
            user_count: row.try_get("user_count").map_err(db_err)?,
            document_count: row.try_get("document_count").map_err(db_err)?,
            query_count: row.try_get("query_count").map_err(db_err)?,
        })
    }

    fn map_user_info(row: sqlx::postgres::PgRow) -> Result<AdminUserInfo, AppError> {
        let id: Uuid = row.try_get("id").map_err(db_err)?;
        let created_at: DateTime<Utc> = row.try_get("created_at").map_err(db_err)?;
        Ok(AdminUserInfo {
            id: UserId::from(id),
            email: row.try_get("email").map_err(db_err)?,
            role: row.try_get("role").map_err(db_err)?,
            created_at: created_at.timestamp(),
        })
    }

    fn build_audit_log_base_query(
        builder: &mut QueryBuilder<'_, Postgres>,
        query: &AdminAuditLogQuery,
        count_only: bool,
    ) {
        if count_only {
            builder.push("select count(*) as total from audit_log where 1 = 1");
        } else {
            builder.push(
                "select id, actor_id, action, resource_type, resource_id, owner_user_id, created_at from audit_log where 1 = 1",
            );
        }

        if let Some(window_start) = admin_audit_window_start(query.window.as_deref()) {
            builder.push(" and created_at >= ").push_bind(window_start);
        }
        if let Some(action) = query.action.as_deref() {
            builder
                .push(" and action = ")
                .push_bind(action.trim().to_string());
        }
        if let Some(resource_type) = query.resource_type.as_deref() {
            builder
                .push(" and resource_type = ")
                .push_bind(resource_type.trim().to_string());
        }
        if let Some(actor) = query.actor.as_deref() {
            let pattern = format!("%{}%", admin_escape_ilike_pattern(actor.trim()));
            builder
                .push(" and coalesce(actor_id::text, '') ilike ")
                .push_bind(pattern);
            builder.push(" escape '\\'");
        }
        if let Some(search) = query.query.as_deref() {
            let pattern = format!("%{}%", admin_escape_ilike_pattern(search.trim()));
            builder.push(" and (action ilike ");
            builder.push_bind(pattern.clone());
            builder.push(" escape '\\' or resource_type ilike ");
            builder.push_bind(pattern.clone());
            builder.push(" escape '\\' or resource_id ilike ");
            builder.push_bind(pattern.clone());
            builder.push(" escape '\\' or coalesce(actor_id::text, '') ilike ");
            builder.push_bind(pattern);
            builder.push(" escape '\\')");
        }
    }

    async fn audit_log_total(
        conn: &mut sqlx::PgConnection,
        query: &AdminAuditLogQuery,
    ) -> Result<usize, AppError> {
        let mut builder = QueryBuilder::<Postgres>::new("");
        Self::build_audit_log_base_query(&mut builder, query, true);
        let row = builder.build().fetch_one(conn).await.map_err(db_err)?;
        Ok(row
            .try_get::<i64, _>("total")
            .map_err(db_err)?
            .max(0) as usize)
    }

    async fn audit_log_rows(
        conn: &mut sqlx::PgConnection,
        query: &AdminAuditLogQuery,
    ) -> Result<Vec<sqlx::postgres::PgRow>, AppError> {
        let per_page = admin_clamp_audit_per_page(query.per_page);
        let page = query.page.max(1);
        let offset = (page - 1) * per_page;
        let mut builder = QueryBuilder::<Postgres>::new("");
        Self::build_audit_log_base_query(&mut builder, query, false);
        builder.push(" order by created_at desc, id desc limit ");
        builder.push_bind(per_page as i64);
        builder.push(" offset ");
        builder.push_bind(offset as i64);
        builder.build().fetch_all(conn).await.map_err(db_err)
    }

    fn map_audit_log_entry(row: sqlx::postgres::PgRow) -> Result<AdminAuditLogEntry, AppError> {
        let actor_id = row
            .try_get::<Option<Uuid>, _>("actor_id")
            .map_err(db_err)?;
        let owner_user_id = row
            .try_get::<Option<Uuid>, _>("owner_user_id")
            .map_err(db_err)?;
        let created_at: DateTime<Utc> = row.try_get("created_at").map_err(db_err)?;
        Ok(AdminAuditLogEntry {
            id: row.try_get("id").map_err(db_err)?,
            actor_id: actor_id.map(|value| value.to_string()),
            action: row.try_get("action").map_err(db_err)?,
            resource_type: row.try_get("resource_type").map_err(db_err)?,
            resource_id: row.try_get("resource_id").map_err(db_err)?,
            owner_user_id: owner_user_id.map(|value| value.to_string()),
            created_at: created_at.timestamp(),
        })
    }
}
