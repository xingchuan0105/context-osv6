    async fn count_share_enabled_workspaces(
        &self,
        auth: &AuthContext,
        owner_user_id: Uuid,
    ) -> Result<i64, AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_rls_owner(tx.as_mut(), &auth.user_id().to_string()).await?;
        let row = sqlx::query(
            r#"
            select count(*)::bigint as total
            from workspaces
            where owner_id = $1
              and share_enabled = true
            "#,
        )
        .bind(owner_user_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(row.try_get::<i64, _>("total").unwrap_or(0))
    }

    async fn owner_plan_id(
        &self,
        _auth: &AuthContext,
        owner_user_id: Uuid,
    ) -> Result<String, AppError> {
        // Plan rows are global policy; no RLS on subscriptions.
        let plan_row = sqlx::query(
            r#"
            select plan_id
            from subscriptions
            where user_id = $1 and status = 'active'
            order by updated_at desc
            limit 1
            "#,
        )
        .bind(owner_user_id)
        .fetch_optional(self.repo.raw())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(plan_row
            .and_then(|row| row.try_get::<String, _>("plan_id").ok())
            .unwrap_or_else(|| "free".to_string()))
    }

    async fn max_shared_workspaces_for_owner(
        &self,
        auth: &AuthContext,
        owner_user_id: Uuid,
    ) -> Result<i32, AppError> {
        let plan_id = self.owner_plan_id(auth, owner_user_id).await?;

        let policy_row = sqlx::query(
            r#"
            select max_shared_workspaces
            from usage_limit_plan_policies
            where plan_id = $1
            "#,
        )
        .bind(&plan_id)
        .fetch_optional(self.repo.raw())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;

        Ok(policy_row
            .and_then(|row| row.try_get::<i32, _>("max_shared_workspaces").ok())
            .unwrap_or_else(|| {
                // Fallback mirrors ADR-0010 defaults when policy row is missing.
                match plan_id.as_str() {
                    "plus" | "starter" | "team" | "enterprise" => 10,
                    "pro" => 100,
                    _ => 3,
                }
            }))
    }

    async fn set_share_enabled(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        enabled: bool,
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
            update workspaces
            set share_enabled = $2, updated_at = now()
            where id = $1
            "#,
        )
        .bind(workspace_id)
        .bind(enabled)
        .execute(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(())
    }
