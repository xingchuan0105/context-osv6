#[async_trait]
impl ShareStorePort for PgShareStoreAdapter {
    async fn query_notebook_access(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
    ) -> Result<Option<NotebookAccessSnapshot>, AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_org(tx.as_mut(), &auth.org_id().to_string()).await?;
        let row = sqlx::query(
            r#"
            select owner_id, access_level
            from notebooks
            where id = $1 and org_id = $2
            "#,
        )
        .bind(notebook_id)
        .bind(auth.org_id().into_uuid())
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(row.map(|row| NotebookAccessSnapshot {
            owner_id: row.try_get::<Option<Uuid>, _>("owner_id").ok().flatten(),
            notebook_access_level: row
                .try_get::<String, _>("access_level")
                .unwrap_or_else(|_| "private".to_string()),
        }))
    }

    async fn query_member_access(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<String>, AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_org(tx.as_mut(), &auth.org_id().to_string()).await?;
        let row = sqlx::query(
            r#"
            select access_level
            from notebook_members
            where org_id = $1 and notebook_id = $2 and user_id = $3 and invite_status = 'accepted'
            "#,
        )
        .bind(auth.org_id().into_uuid())
        .bind(notebook_id)
        .bind(user_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(row.and_then(|row| row.try_get::<String, _>("access_level").ok()))
    }
    async fn get_share_settings(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
    ) -> Result<(String, bool, Vec<ShareTokenSnapshot>), AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_org(tx.as_mut(), &auth.org_id().to_string()).await?;
        let notebook_row =
            sqlx::query("select access_level, allow_download from notebooks where id = $1")
                .bind(notebook_id)
                .fetch_one(tx.as_mut())
                .await
                .map_err(|error| AppError::internal(error.to_string()))?;
        let access_level = notebook_row
            .try_get::<String, _>("access_level")
            .unwrap_or_else(|_| "private".to_string());
        let allow_download = notebook_row
            .try_get::<bool, _>("allow_download")
            .unwrap_or(false);
        let share_tokens = sqlx::query(
            r#"
            select token, access_level, expires_at, revoked_at, access_count
            from share_tokens
            where org_id = $1 and notebook_id = $2
            order by created_at desc
            "#,
        )
        .bind(auth.org_id().into_uuid())
        .bind(notebook_id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok((
            access_level,
            allow_download,
            share_tokens
                .into_iter()
                .map(|row| ShareTokenSnapshot {
                    token: row.try_get("token").unwrap_or_default(),
                    access_level: row.try_get("access_level").unwrap_or_default(),
                    expires_at: row
                        .try_get::<Option<DateTime<Utc>>, _>("expires_at")
                        .ok()
                        .flatten()
                        .map(|value| value.to_rfc3339()),
                    revoked_at: row
                        .try_get::<Option<DateTime<Utc>>, _>("revoked_at")
                        .ok()
                        .flatten()
                        .map(|value| value.to_rfc3339()),
                    access_count: i64::from(
                        row.try_get::<i32, _>("access_count").unwrap_or_default(),
                    ),
                })
                .collect(),
        ))
    }

    async fn update_notebook_access_level(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
        access_level: &str,
    ) -> Result<(), AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_org(tx.as_mut(), &auth.org_id().to_string()).await?;
        sqlx::query("update notebooks set access_level = $2, updated_at = now() where id = $1")
            .bind(notebook_id)
            .bind(access_level)
            .execute(tx.as_mut())
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(())
    }

    async fn update_share_settings(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
        access_level: Option<&str>,
        allow_download: Option<bool>,
    ) -> Result<(), AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_org(tx.as_mut(), &auth.org_id().to_string()).await?;
        sqlx::query(
            r#"
            update notebooks
            set access_level = coalesce($2, access_level),
                allow_download = coalesce($3, allow_download),
                updated_at = now()
            where id = $1
            "#,
        )
        .bind(notebook_id)
        .bind(access_level)
        .bind(allow_download)
        .execute(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(())
    }
    async fn list_members(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
    ) -> Result<Vec<ShareNotebookMember>, AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_org(tx.as_mut(), &auth.org_id().to_string()).await?;
        let rows = sqlx::query(
            r#"
            select id, notebook_id, user_id, email, access_level, invite_status, invited_by, invited_at, accepted_at
            from notebook_members
            where org_id = $1 and notebook_id = $2
            order by invited_at asc
            "#,
        )
        .bind(auth.org_id().into_uuid())
        .bind(notebook_id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        rows.into_iter().map(map_member).collect()
    }
    async fn create_share_token(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
        access_level: ShareAccessLevel,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<String, AppError> {
        let token = Uuid::new_v4().to_string();
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_org(tx.as_mut(), &auth.org_id().to_string()).await?;
        sqlx::query(
            r#"
            insert into share_tokens (token, org_id, notebook_id, access_level, created_by, expires_at)
            values ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(&token)
        .bind(auth.org_id().into_uuid())
        .bind(notebook_id)
        .bind(access_level.as_db())
        .bind(auth.actor_id().map(|id| id.into_uuid()))
        .bind(expires_at)
        .execute(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(token)
    }

    async fn validate_token(
        &self,
        token: &str,
    ) -> Result<Option<(Uuid, ShareAccessLevel)>, AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_role(tx.as_mut(), "super_admin").await?;
        set_public_share_token(tx.as_mut(), token).await?;
        let row = sqlx::query(
            r#"
            select notebook_id, access_level
            from share_tokens
            where token = $1
              and revoked_at is null
              and (expires_at is null or expires_at > now())
            "#,
        )
        .bind(token)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(row.map(|row| {
            (
                row.try_get::<Uuid, _>("notebook_id").unwrap_or_default(),
                row.try_get::<String, _>("access_level")
                    .map(|role| ShareAccessLevel::from_role(&role))
                    .unwrap_or(ShareAccessLevel::None),
            )
        }))
    }

    async fn revoke_token(
        &self,
        auth: &AuthContext,
        token: &str,
    ) -> Result<Option<Uuid>, AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_public_share_token(tx.as_mut(), token).await?;
        let row = sqlx::query(
            "select notebook_id from share_tokens where token = $1 and revoked_at is null",
        )
        .bind(token)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        let Some(row) = row else {
            tx.rollback()
                .await
                .map_err(|error| AppError::internal(error.to_string()))?;
            return Ok(None);
        };
        let notebook_id = row
            .try_get::<Uuid, _>("notebook_id")
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_org(tx.as_mut(), &auth.org_id().to_string()).await?;
        sqlx::query("update share_tokens set revoked_at = now() where token = $1")
            .bind(token)
            .execute(tx.as_mut())
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(Some(notebook_id))
    }
    async fn get_share_analytics(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
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
            where st.org_id = $1 and st.notebook_id = $2
            group by st.token, st.access_level, st.created_at
            order by total_views desc, max(sal.created_at) desc nulls last
            "#,
        )
        .bind(auth.org_id().into_uuid())
        .bind(notebook_id)
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
        notebook_id: Uuid,
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
            select sal.id, sal.notebook_id, sal.share_token, sal.action, sal.created_at
            from share_access_logs sal
            join share_tokens st on st.token = sal.share_token
            where st.org_id = $1 and st.notebook_id = $2
            order by sal.created_at desc
            limit $3
            "#,
        )
        .bind(auth.org_id().into_uuid())
        .bind(notebook_id)
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
                notebook_id: row
                    .try_get::<Uuid, _>("notebook_id")
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
    async fn load_shared_notebook(
        &self,
        token: &str,
    ) -> Result<Option<SharedNotebookSnapshot>, AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_role(tx.as_mut(), "super_admin").await?;
        set_public_share_token(tx.as_mut(), token).await?;
        let row = sqlx::query(
            r#"
            select
              st.org_id,
              st.notebook_id,
              st.access_level,
              st.expires_at,
              n.allow_download
            from share_tokens st
            join notebooks n on n.id = st.notebook_id
            where st.token = $1
              and st.revoked_at is null
              and (st.expires_at is null or st.expires_at > now())
            "#,
        )
        .bind(token)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        let Some(row) = row else {
            tx.rollback()
                .await
                .map_err(|error| AppError::internal(error.to_string()))?;
            return Ok(None);
        };
        let org_id = row
            .try_get::<Uuid, _>("org_id")
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_org(tx.as_mut(), &org_id.to_string()).await?;
        let notebook_id = row
            .try_get::<Uuid, _>("notebook_id")
            .map_err(|error| AppError::internal(error.to_string()))?;
        let access_level = row
            .try_get::<String, _>("access_level")
            .map_err(|error| AppError::internal(error.to_string()))?;
        let expires_at = row
            .try_get::<Option<DateTime<Utc>>, _>("expires_at")
            .ok()
            .flatten();
        let allow_download = row.try_get::<bool, _>("allow_download").unwrap_or(false);
        sqlx::query("update share_tokens set access_count = access_count + 1 where token = $1")
            .bind(token)
            .execute(tx.as_mut())
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        sqlx::query(
            r#"
            insert into share_access_logs (org_id, notebook_id, share_token, action, created_at)
            values ($1, $2, $3, 'view', now())
            "#,
        )
        .bind(org_id)
        .bind(notebook_id)
        .bind(token)
        .execute(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        let notebook_row = sqlx::query("select title, description from notebooks where id = $1")
            .bind(notebook_id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        let title = notebook_row
            .try_get::<String, _>("title")
            .map_err(|error| AppError::internal(error.to_string()))?;
        let description = notebook_row.try_get::<String, _>("description").ok();
        let sources_rows = sqlx::query(
            r#"
            select id, file_name, status
            from documents
            where notebook_id = $1
              and status not in ('deleting', 'deleted')
            order by updated_at desc, created_at desc
            "#,
        )
        .bind(notebook_id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(Some(SharedNotebookSnapshot {
            knowledge_base: SharedKnowledgeBaseSnapshot {
                id: notebook_id.to_string(),
                title,
                description,
            },
            share: SharedShareInfoSnapshot {
                permission: ShareAccessLevel::from_role(&access_level)
                    .as_permission_label()
                    .to_string(),
                expires_at: expires_at.map(|dt| dt.to_rfc3339()),
                allow_download,
                scope: "full".to_string(),
            },
            sources: sources_rows
                .into_iter()
                .map(|row| SharedSourceSnapshot {
                    id: row
                        .try_get::<Uuid, _>("id")
                        .map(|id| id.to_string())
                        .unwrap_or_default(),
                    file_name: row.try_get("file_name").unwrap_or_default(),
                    status: row.try_get("status").unwrap_or_default(),
                })
                .collect(),
        }))
    }

    async fn resolve_public_share_chat_context(
        &self,
        token: &str,
    ) -> Result<Option<PublicShareChatContextSnapshot>, AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_role(tx.as_mut(), "super_admin").await?;
        set_public_share_token(tx.as_mut(), token).await?;
        let row = sqlx::query(
            r#"
            select
              st.org_id,
              st.notebook_id,
              st.access_level,
              coalesce(n.owner_id, st.created_by) as owner_user_id
            from share_tokens st
            join notebooks n on n.id = st.notebook_id
            where st.token = $1
              and st.revoked_at is null
              and (st.expires_at is null or st.expires_at > now())
            "#,
        )
        .bind(token)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        let Some(row) = row else {
            tx.rollback()
                .await
                .map_err(|error| AppError::internal(error.to_string()))?;
            return Ok(None);
        };
        let org_id = row
            .try_get::<Uuid, _>("org_id")
            .map_err(|error| AppError::internal(error.to_string()))?;
        let notebook_id = row
            .try_get::<Uuid, _>("notebook_id")
            .map_err(|error| AppError::internal(error.to_string()))?;
        let access_level = row
            .try_get::<String, _>("access_level")
            .map_err(|error| AppError::internal(error.to_string()))?;
        let owner_user_id = row
            .try_get::<Option<Uuid>, _>("owner_user_id")
            .map_err(|error| AppError::internal(error.to_string()))?;
        let Some(owner_user_id) = owner_user_id else {
            tx.rollback()
                .await
                .map_err(|error| AppError::internal(error.to_string()))?;
            return Ok(None);
        };
        sqlx::query(
            r#"
            insert into share_access_logs (org_id, notebook_id, share_token, action, created_at)
            values ($1, $2, $3, 'chat', now())
            "#,
        )
        .bind(org_id)
        .bind(notebook_id)
        .bind(token)
        .execute(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(Some(PublicShareChatContextSnapshot {
            org_id,
            notebook_id,
            owner_user_id,
            access_level: ShareAccessLevel::from_role(&access_level),
        }))
    }
    async fn invite_member(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
        email: &str,
        access_level: ShareAccessLevel,
    ) -> Result<ShareNotebookMember, AppError> {
        let normalized_email = email.trim().to_lowercase();
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_org(tx.as_mut(), &auth.org_id().to_string()).await?;
        let invited_user = sqlx::query(
            "select id from users where org_id = $1 and lower(email) = lower($2) limit 1",
        )
        .bind(auth.org_id().into_uuid())
        .bind(&normalized_email)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        let user_id = invited_user.and_then(|row| row.try_get::<Uuid, _>("id").ok());
        let existing = sqlx::query(
            "select id from notebook_members where org_id = $1 and notebook_id = $2 and lower(email) = lower($3) limit 1",
        )
        .bind(auth.org_id().into_uuid())
        .bind(notebook_id)
        .bind(&normalized_email)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        let row = if let Some(existing) = existing {
            sqlx::query(
                r#"
                update notebook_members
                set user_id = $4,
                    access_level = $5,
                    invited_by = $6,
                    invite_status = 'pending',
                    invited_at = now(),
                    updated_at = now(),
                    accepted_at = null
                where id = $1 and org_id = $2 and notebook_id = $3
                returning id, notebook_id, user_id, email, access_level, invite_status, invited_by, invited_at, accepted_at
                "#,
            )
            .bind(existing.try_get::<Uuid, _>("id").map_err(|error| AppError::internal(error.to_string()))?)
            .bind(auth.org_id().into_uuid())
            .bind(notebook_id)
            .bind(user_id)
            .bind(access_level.as_db())
            .bind(auth.actor_id().map(|id| id.into_uuid()))
            .fetch_one(tx.as_mut())
            .await
            .map_err(|error| AppError::internal(error.to_string()))?
        } else {
            sqlx::query(
                r#"
                insert into notebook_members (id, org_id, notebook_id, user_id, email, access_level, invited_by, invite_status, invited_at, updated_at)
                values ($1, $2, $3, $4, $5, $6, $7, 'pending', now(), now())
                returning id, notebook_id, user_id, email, access_level, invite_status, invited_by, invited_at, accepted_at
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(auth.org_id().into_uuid())
            .bind(notebook_id)
            .bind(user_id)
            .bind(&normalized_email)
            .bind(access_level.as_db())
            .bind(auth.actor_id().map(|id| id.into_uuid()))
            .fetch_one(tx.as_mut())
            .await
            .map_err(|error| AppError::internal(error.to_string()))?
        };
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        map_member(row)
    }

    async fn accept_invite(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
        member_id: Uuid,
        actor_id: Uuid,
    ) -> Result<(), AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_org(tx.as_mut(), &auth.org_id().to_string()).await?;
        let actor_email = sqlx::query(
            "select lower(email) as email from users where id = $1 and org_id = $2",
        )
        .bind(actor_id)
        .bind(auth.org_id().into_uuid())
        .fetch_one(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?
        .try_get::<String, _>("email")
        .map_err(|error| AppError::internal(error.to_string()))?;
        let row = sqlx::query(
            r#"
            select email, invite_status
            from notebook_members
            where id = $1 and org_id = $2 and notebook_id = $3
            for update
            "#,
        )
        .bind(member_id)
        .bind(auth.org_id().into_uuid())
        .bind(notebook_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?
        .ok_or_else(|| AppError::not_found("invite_not_found", "invite not found"))?;
        let invite_email = row
            .try_get::<Option<String>, _>("email")
            .ok()
            .flatten()
            .unwrap_or_default()
            .to_lowercase();
        let invite_status = row
            .try_get::<String, _>("invite_status")
            .map_err(|error| AppError::internal(error.to_string()))?;
        if invite_status != "pending" || (!invite_email.is_empty() && invite_email != actor_email)
        {
            tx.rollback()
                .await
                .map_err(|error| AppError::internal(error.to_string()))?;
            return Err(AppError::validation("invite_not_allowed", "invite not allowed"));
        }
        sqlx::query(
            r#"
            update notebook_members
            set user_id = $4,
                invite_status = 'accepted',
                accepted_at = now(),
                updated_at = now()
            where id = $1 and org_id = $2 and notebook_id = $3
            "#,
        )
        .bind(member_id)
        .bind(auth.org_id().into_uuid())
        .bind(notebook_id)
        .bind(actor_id)
        .execute(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(())
    }

    async fn decline_invite(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
        member_id: Uuid,
        actor_id: Uuid,
    ) -> Result<(), AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_org(tx.as_mut(), &auth.org_id().to_string()).await?;
        let actor_email = sqlx::query(
            "select lower(email) as email from users where id = $1 and org_id = $2",
        )
        .bind(actor_id)
        .bind(auth.org_id().into_uuid())
        .fetch_one(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?
        .try_get::<String, _>("email")
        .map_err(|error| AppError::internal(error.to_string()))?;
        let row = sqlx::query(
            r#"
            select email, invite_status
            from notebook_members
            where id = $1 and org_id = $2 and notebook_id = $3
            for update
            "#,
        )
        .bind(member_id)
        .bind(auth.org_id().into_uuid())
        .bind(notebook_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?
        .ok_or_else(|| AppError::not_found("invite_not_found", "invite not found"))?;
        let invite_email = row
            .try_get::<Option<String>, _>("email")
            .ok()
            .flatten()
            .unwrap_or_default()
            .to_lowercase();
        let invite_status = row
            .try_get::<String, _>("invite_status")
            .map_err(|error| AppError::internal(error.to_string()))?;
        if invite_status != "pending" || (!invite_email.is_empty() && invite_email != actor_email)
        {
            tx.rollback()
                .await
                .map_err(|error| AppError::internal(error.to_string()))?;
            return Err(AppError::validation("invite_not_allowed", "invite not allowed"));
        }
        sqlx::query(
            r#"
            update notebook_members
            set invite_status = 'declined',
                updated_at = now()
            where id = $1 and org_id = $2 and notebook_id = $3
            "#,
        )
        .bind(member_id)
        .bind(auth.org_id().into_uuid())
        .bind(notebook_id)
        .execute(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(())
    }

    async fn add_member(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
        user_id: Uuid,
        access_level: ShareAccessLevel,
    ) -> Result<(), AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_org(tx.as_mut(), &auth.org_id().to_string()).await?;
        sqlx::query(
            r#"
            insert into notebook_members (id, org_id, notebook_id, user_id, access_level, invited_by, invite_status, invited_at, accepted_at, updated_at)
            values ($1, $2, $3, $4, $5, $6, 'accepted', now(), now(), now())
            on conflict (notebook_id, user_id) do update
            set access_level = excluded.access_level,
                invited_by = excluded.invited_by,
                invite_status = 'accepted',
                invited_at = now(),
                accepted_at = now(),
                updated_at = now()
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(auth.org_id().into_uuid())
        .bind(notebook_id)
        .bind(user_id)
        .bind(access_level.as_db())
        .bind(auth.actor_id().map(|id| id.into_uuid()))
        .execute(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(())
    }

    async fn remove_member(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
        member_id: Uuid,
    ) -> Result<(), AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_org(tx.as_mut(), &auth.org_id().to_string()).await?;
        sqlx::query(
            r#"
            delete from notebook_members
            where org_id = $1 and notebook_id = $2 and id = $3
            "#,
        )
        .bind(auth.org_id().into_uuid())
        .bind(notebook_id)
        .bind(member_id)
        .execute(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(())
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
}
