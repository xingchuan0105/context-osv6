    async fn invite_member(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        email: &str,
        access_level: ShareAccessLevel,
    ) -> Result<ShareWorkspaceMember, AppError> {
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
            "select id from workspace_members where org_id = $1 and workspace_id = $2 and lower(email) = lower($3) limit 1",
        )
        .bind(auth.org_id().into_uuid())
        .bind(workspace_id)
        .bind(&normalized_email)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        let row = if let Some(existing) = existing {
            sqlx::query(
                r#"
                update workspace_members
                set user_id = $4,
                    access_level = $5,
                    invited_by = $6,
                    invite_status = 'pending',
                    invited_at = now(),
                    updated_at = now(),
                    accepted_at = null
                where id = $1 and org_id = $2 and workspace_id = $3
                returning id, workspace_id, user_id, email, access_level, invite_status, invited_by, invited_at, accepted_at
                "#,
            )
            .bind(existing.try_get::<Uuid, _>("id").map_err(|error| AppError::internal(error.to_string()))?)
            .bind(auth.org_id().into_uuid())
            .bind(workspace_id)
            .bind(user_id)
            .bind(access_level.as_db())
            .bind(auth.actor_id().map(|id| id.into_uuid()))
            .fetch_one(tx.as_mut())
            .await
            .map_err(|error| AppError::internal(error.to_string()))?
        } else {
            sqlx::query(
                r#"
                insert into workspace_members (id, org_id, workspace_id, user_id, email, access_level, invited_by, invite_status, invited_at, updated_at)
                values ($1, $2, $3, $4, $5, $6, $7, 'pending', now(), now())
                returning id, workspace_id, user_id, email, access_level, invite_status, invited_by, invited_at, accepted_at
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(auth.org_id().into_uuid())
            .bind(workspace_id)
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
