    async fn decline_invite(
        &self,
        _auth: &AuthContext,
        workspace_id: Uuid,
        member_id: Uuid,
        actor_id: Uuid,
    ) -> Result<(), AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        // Invitee is not workspace owner — elevate for cross-user member row access.
        set_current_role(tx.as_mut(), "super_admin").await?;
        let actor_email = sqlx::query(
            "select lower(email) as email from users where id = $1",
        )
        .bind(actor_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?
        .ok_or_else(|| AppError::not_found("actor_not_found", "user not found"))?
        .try_get::<String, _>("email")
        .map_err(|error| AppError::internal(error.to_string()))?;
        let row = sqlx::query(
            r#"
            select email, invite_status, owner_user_id
            from workspace_members
            where id = $1 and workspace_id = $2
            for update
            "#,
        )
        .bind(member_id)
        .bind(workspace_id)
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
        let owner_user_id = row
            .try_get::<Uuid, _>("owner_user_id")
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
            update workspace_members
            set invite_status = 'declined',
                updated_at = now()
            where id = $1 and owner_user_id = $2 and workspace_id = $3
            "#,
        )
        .bind(member_id)
        .bind(owner_user_id)
        .bind(workspace_id)
        .execute(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(())
    }
