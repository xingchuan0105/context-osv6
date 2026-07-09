    async fn create_share_token(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
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
            insert into share_tokens (token, org_id, workspace_id, access_level, created_by, expires_at)
            values ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(&token)
        .bind(auth.org_id().into_uuid())
        .bind(workspace_id)
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
            select workspace_id, access_level
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
                row.try_get::<Uuid, _>("workspace_id").unwrap_or_default(),
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
            "select workspace_id from share_tokens where token = $1 and revoked_at is null",
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
        let workspace_id = row
            .try_get::<Uuid, _>("workspace_id")
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
        Ok(Some(workspace_id))
    }
