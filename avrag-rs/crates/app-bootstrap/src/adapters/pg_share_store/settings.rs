    async fn get_share_settings(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<(String, bool, Vec<ShareTokenSnapshot>), AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_org(tx.as_mut(), &auth.org_id().to_string()).await?;
        let notebook_row =
            sqlx::query("select access_level, allow_download from workspaces where id = $1")
                .bind(workspace_id)
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
            where org_id = $1 and workspace_id = $2
            order by created_at desc
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
        workspace_id: Uuid,
        access_level: &str,
    ) -> Result<(), AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_org(tx.as_mut(), &auth.org_id().to_string()).await?;
        sqlx::query("update workspaces set access_level = $2, updated_at = now() where id = $1")
            .bind(workspace_id)
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
        workspace_id: Uuid,
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
        .bind(workspace_id)
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
        workspace_id: Uuid,
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
            select id, workspace_id, user_id, email, access_level, invite_status, invited_by, invited_at, accepted_at
            from workspace_members
            where org_id = $1 and workspace_id = $2
            order by invited_at asc
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
        rows.into_iter().map(map_member).collect()
    }
