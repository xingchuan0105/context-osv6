use super::*;
impl AuthRepository {
    pub async fn list_api_keys(
        &self,
        context: &AuthContext,
        notebook_id: Option<Uuid>,
    ) -> Result<Vec<ApiKeyRow>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select id, org_id, notebook_id, key_prefix, name, permissions, rate_limit_rpm,
                   expires_at, last_used_at, is_active, created_by, created_at, updated_at
            from api_keys
            where is_active = true
              and (
                ($1::uuid is not null and notebook_id = $1)
                or ($1::uuid is null and notebook_id is null)
              )
            order by created_at desc
            "#,
        )
        .bind(notebook_id)
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;
        rows.into_iter().map(map_api_key).collect()
    }

    pub async fn create_api_key(
        &self,
        context: &AuthContext,
        notebook_id: Option<Uuid>,
        name: &str,
        permissions: &[String],
        rate_limit_rpm: u32,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<(ApiKeyRow, String), PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;
        if let Some(notebook_id) = notebook_id {
            let exists = sqlx::query("select 1 from notebooks where id = $1")
                .bind(notebook_id)
                .fetch_optional(tx.inner())
                .await?;
            if exists.is_none() {
                tx.rollback().await?;
                return Err(PgStorageError::NotFound("notebook not found".to_string()));
            }
        }

        let plaintext_key = generate_plaintext_api_key();
        let key_hash = hash_api_key(&plaintext_key);
        let key_prefix = plaintext_key.chars().take(12).collect::<String>();
        let row = sqlx::query(
            r#"
            insert into api_keys (
                org_id, notebook_id, key_hash, key_prefix, name, permissions, rate_limit_rpm, expires_at, created_by
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            returning id, org_id, notebook_id, key_prefix, name, permissions, rate_limit_rpm,
                      expires_at, last_used_at, is_active, created_by, created_at, updated_at
            "#,
        )
        .bind(context.org_id().into_uuid())
        .bind(notebook_id)
        .bind(key_hash)
        .bind(key_prefix)
        .bind(name)
        .bind(contracts::normalize_api_key_permissions(permissions, notebook_id))
        .bind(i32::try_from(rate_limit_rpm).unwrap_or(i32::MAX))
        .bind(expires_at)
        .bind(context.actor_id().map(ActorId::into_uuid))
        .fetch_one(tx.inner())
        .await?;
        tx.commit().await?;
        Ok((map_api_key(row)?, plaintext_key))
    }

    pub async fn revoke_api_key(
        &self,
        context: &AuthContext,
        notebook_id: Option<Uuid>,
        key_id: Uuid,
    ) -> Result<bool, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let result = sqlx::query(
            r#"
            update api_keys
            set is_active = false, updated_at = now()
            where id = $1
              and ($2::uuid is null or notebook_id = $2)
            "#,
        )
        .bind(key_id)
        .bind(notebook_id)
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_notifications(
        &self,
        context: &AuthContext,
        user_id: Uuid,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<NotificationRow>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select id, org_id, user_id, event_type, title, body, data, read_at, created_at, updated_at
            from notifications
            where user_id = $1
            order by created_at desc
            limit $2 offset $3
            "#,
        )
        .bind(user_id)
        .bind(i64::try_from(limit.max(1)).unwrap_or(i64::MAX))
        .bind(i64::try_from(offset).unwrap_or_default())
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;
        rows.into_iter().map(map_notification).collect()
    }

    pub async fn mark_notification_read(
        &self,
        context: &AuthContext,
        user_id: Uuid,
        notification_id: Uuid,
    ) -> Result<bool, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let result = sqlx::query(
            r#"
            update notifications
            set read_at = coalesce(read_at, now()),
                updated_at = now()
            where id = $1 and user_id = $2
            "#,
        )
        .bind(notification_id)
        .bind(user_id)
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn create_notification(
        &self,
        context: &AuthContext,
        params: NotificationCreateParams,
    ) -> Result<NotificationRow, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;
        let row = insert_notification_row(tx.inner(), context.org_id().into_uuid(), params).await?;
        tx.commit().await?;
        map_notification(row)
    }

    pub async fn create_notifications_for_all_users(
        &self,
        org_id: OrgId,
        event_type: &str,
        title: &str,
        body: &str,
        data: serde_json::Value,
    ) -> Result<usize, PgStorageError> {
        let context = AuthContext::new(org_id, contracts::auth_runtime::SubjectKind::System);
        let mut tx = self.pool.begin(&context).await?;
        let users = sqlx::query("select id from users where org_id = $1")
            .bind(org_id.into_uuid())
            .fetch_all(tx.inner())
            .await?;
        let mut created = 0usize;
        for row in users {
            let user_id: Uuid = row.try_get("id")?;
            let params = NotificationCreateParams {
                user_id,
                event_type: event_type.to_string(),
                title: title.to_string(),
                body: body.to_string(),
                data: data.clone(),
                channels: vec!["in_app".to_string()],
            };
            insert_notification_row(tx.inner(), org_id.into_uuid(), params).await?;
            created += 1;
        }
        tx.commit().await?;
        Ok(created)
    }

    pub async fn validate_api_key(
        &self,
        plaintext_key: &str,
    ) -> Result<Option<ValidatedApiKey>, PgStorageError> {
        let mut tx = self.pool.raw().begin().await?;
        set_current_role(tx.as_mut(), "super_admin").await?;
        let row = sqlx::query(
            r#"
            select id, org_id, notebook_id, permissions, created_by, expires_at, is_active, rate_limit_rpm
            from api_keys
            where key_hash = $1
            limit 1
            "#,
        )
        .bind(hash_api_key(plaintext_key))
        .fetch_optional(tx.as_mut())
        .await?;

        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };

        let is_active: bool = row.try_get("is_active")?;
        let expires_at: Option<DateTime<Utc>> = row.try_get("expires_at").ok().flatten();
        if !is_active || expires_at.is_some_and(|value| value < Utc::now()) {
            tx.commit().await?;
            return Ok(None);
        }

        let id: Uuid = row.try_get("id")?;
        let org_id: Uuid = row.try_get("org_id")?;
        let notebook_id: Option<Uuid> = row.try_get("notebook_id").ok().flatten();
        let permissions = contracts::normalize_api_key_permissions(
            &row
                .try_get::<Vec<String>, _>("permissions")
                .unwrap_or_else(|_| vec![contracts::PERM_QUERY.to_string()]),
            notebook_id,
        );
        let created_by: Option<Uuid> = row.try_get("created_by").ok().flatten();
        let rate_limit_rpm = u32::try_from(row.try_get::<i32, _>("rate_limit_rpm")?).unwrap_or(60);

        sqlx::query(
            r#"
            update api_keys
            set last_used_at = now(),
                updated_at = now()
            where id = $1
            "#,
        )
        .bind(id)
        .execute(tx.as_mut())
        .await?;

        tx.commit().await?;
        Ok(Some(ValidatedApiKey {
            id,
            org_id: OrgId::from(org_id),
            notebook_id,
            permissions,
            created_by,
            rate_limit_rpm,
        }))
    }

    pub async fn get_user_profile(
        &self,
        context: &AuthContext,
        user_id: Uuid,
    ) -> Result<Option<UserProfileRow>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            select user_id, org_id, expertise_domains, preferred_answer_style, frequently_asked_topics,
                   custom_preferences, structured_profile, inferred_at, inference_version
            from user_profiles
            where user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(tx.inner())
        .await?;
        tx.commit().await?;
        row.map(map_user_profile).transpose()
    }

    pub async fn upsert_user_profile(
        &self,
        context: &AuthContext,
        profile: &UserProfileRow,
    ) -> Result<(), PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;
        sqlx::query(
            r#"
            insert into user_profiles (
                user_id, org_id, expertise_domains, preferred_answer_style, frequently_asked_topics,
                custom_preferences, structured_profile, inferred_at, inference_version
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            on conflict (user_id) do update
            set expertise_domains = excluded.expertise_domains,
                preferred_answer_style = excluded.preferred_answer_style,
                frequently_asked_topics = excluded.frequently_asked_topics,
                custom_preferences = excluded.custom_preferences,
                structured_profile = excluded.structured_profile,
                inferred_at = excluded.inferred_at,
                inference_version = excluded.inference_version,
                updated_at = now()
            "#,
        )
        .bind(profile.user_id)
        .bind(profile.org_id.into_uuid())
        .bind(serde_json::to_value(&profile.expertise_domains)?)
        .bind(&profile.preferred_answer_style)
        .bind(serde_json::to_value(&profile.frequently_asked_topics)?)
        .bind(&profile.custom_preferences)
        .bind(&profile.structured_profile)
        .bind(profile.inferred_at)
        .bind(&profile.inference_version)
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn delete_user_cascade(
        &self,
        _context: &AuthContext,
        user_id: Uuid,
    ) -> Result<bool, PgStorageError> {
        let mut tx = self.pool.raw().begin().await?;
        let row = sqlx::query("select delete_user_cascade($1) as deleted")
            .bind(user_id)
            .fetch_one(tx.as_mut())
            .await?;
        let deleted: i64 = row.try_get("deleted")?;
        tx.commit().await?;
        Ok(deleted > 0)
    }

}
