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
