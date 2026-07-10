    async fn query_workspace_access(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<Option<WorkspaceAccessSnapshot>, AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_rls_owner(tx.as_mut(), &auth.user_id().to_string()).await?;
        let row = sqlx::query(
            r#"
            select owner_id, access_level
            from workspaces
            where id = $1 and owner_user_id = $2
            "#,
        )
        .bind(workspace_id)
        .bind(auth.user_id().into_uuid())
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(row.map(|row| WorkspaceAccessSnapshot {
            owner_id: row.try_get::<Option<Uuid>, _>("owner_id").ok().flatten(),
            notebook_access_level: row
                .try_get::<String, _>("access_level")
                .unwrap_or_else(|_| "private".to_string()),
        }))
    }

    async fn query_member_access(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<String>, AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_rls_owner(tx.as_mut(), &auth.user_id().to_string()).await?;
        let row = sqlx::query(
            r#"
            select access_level
            from workspace_members
            where owner_user_id = $1 and workspace_id = $2 and user_id = $3 and invite_status = 'accepted'
            "#,
        )
        .bind(auth.user_id().into_uuid())
        .bind(workspace_id)
        .bind(user_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(row.and_then(|row| row.try_get::<String, _>("access_level").ok()))
    }
