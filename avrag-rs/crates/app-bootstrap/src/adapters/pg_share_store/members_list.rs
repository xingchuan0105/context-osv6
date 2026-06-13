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
