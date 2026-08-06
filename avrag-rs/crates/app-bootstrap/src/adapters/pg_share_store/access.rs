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
            select owner_id, access_level, share_enabled
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
            share_enabled: row.try_get::<bool, _>("share_enabled").unwrap_or(false),
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

    async fn owner_for_accepted_member(
        &self,
        workspace_id: Uuid,
        member_user_id: Uuid,
    ) -> Result<Option<Uuid>, AppError> {
        // Cross-tenant lookup for Owner-pays: must not rely on caller's RLS owner.
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_role(tx.as_mut(), "super_admin").await?;
        let owner = sqlx::query_scalar::<_, Uuid>(
            r#"
            select w.owner_user_id
            from workspaces w
            inner join workspace_members m
              on m.workspace_id = w.id
             and m.owner_user_id = w.owner_user_id
            where w.id = $1
              and m.user_id = $2
              and m.invite_status = 'accepted'
            limit 1
            "#,
        )
        .bind(workspace_id)
        .bind(member_user_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(owner.filter(|o| *o != member_user_id))
    }
