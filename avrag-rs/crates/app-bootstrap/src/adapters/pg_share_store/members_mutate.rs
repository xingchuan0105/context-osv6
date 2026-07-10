    async fn add_member(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        user_id: Uuid,
        access_level: ShareAccessLevel,
    ) -> Result<(), AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_rls_owner(tx.as_mut(), &auth.user_id().to_string()).await?;
        sqlx::query(
            r#"
            insert into workspace_members (id, owner_user_id, workspace_id, user_id, access_level, invited_by, invite_status, invited_at, accepted_at, updated_at)
            values ($1, $2, $3, $4, $5, $6, 'accepted', now(), now(), now())
            on conflict (workspace_id, user_id) do update
            set access_level = excluded.access_level,
                invited_by = excluded.invited_by,
                invite_status = 'accepted',
                invited_at = now(),
                accepted_at = now(),
                updated_at = now()
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(auth.user_id().into_uuid())
        .bind(workspace_id)
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
        workspace_id: Uuid,
        member_id: Uuid,
    ) -> Result<(), AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_rls_owner(tx.as_mut(), &auth.user_id().to_string()).await?;
        sqlx::query(
            r#"
            delete from workspace_members
            where owner_user_id = $1 and workspace_id = $2 and id = $3
            "#,
        )
        .bind(auth.user_id().into_uuid())
        .bind(workspace_id)
        .bind(member_id)
        .execute(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(())
    }
