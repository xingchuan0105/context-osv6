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
