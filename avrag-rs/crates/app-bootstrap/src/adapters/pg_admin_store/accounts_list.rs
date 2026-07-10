    async fn list_accounts(
        &self,
        auth: &AuthContext,
        page: usize,
        per_page: usize,
    ) -> Result<Vec<AdminAccountInfo>, AppError> {
        // B2C: "org" admin surface lists personal accounts (users).
        let mut tx = self.begin_admin_tx(auth).await?;
        let page = page.max(1);
        let per_page = admin_clamp_account_list_per_page(per_page);
        let offset = ((page - 1) * per_page) as i64;
        let rows = sqlx::query(
            r#"
            select
              u.id,
              coalesce(nullif(u.full_name, ''), u.email) as name,
              u.created_at,
              coalesce(u.blocked, false) as blocked,
              1::bigint as user_count,
              (select count(*) from documents d where d.owner_user_id = u.id) as document_count,
              (select count(*) from chat_messages m
                 where m.owner_user_id = u.id and m.role = 'user') as query_count
            from users u
            order by u.created_at desc
            limit $1 offset $2
            "#,
        )
        .bind(per_page as i64)
        .bind(offset)
        .fetch_all(tx.as_mut())
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        rows.into_iter().map(Self::map_account_info).collect()
    }

    async fn get_account(&self, auth: &AuthContext, owner_user_id: UserId) -> Result<AdminAccountInfo, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let row = sqlx::query(
            r#"
            select
              u.id,
              coalesce(nullif(u.full_name, ''), u.email) as name,
              u.created_at,
              coalesce(u.blocked, false) as blocked,
              1::bigint as user_count,
              (select count(*) from documents d where d.owner_user_id = u.id) as document_count,
              (select count(*) from chat_messages m
                 where m.owner_user_id = u.id and m.role = 'user') as query_count
            from users u
            where u.id = $1
            "#,
        )
        .bind(owner_user_id.into_uuid())
        .fetch_optional(tx.as_mut())
        .await
        .map_err(db_err)?;
        tx.commit()
            .await
            .map_err(db_err)?;
        row.map(Self::map_account_info)
            .transpose()?
            .ok_or_else(|| AppError::not_found("account_not_found", "Account not found"))
    }

    async fn list_users(
        &self,
        auth: &AuthContext,
        owner_user_id: UserId,
    ) -> Result<Vec<AdminUserInfo>, AppError> {
        // Personal account: the only user under an owner is the owner themself.
        let mut tx = self.begin_admin_tx(auth).await?;
        let rows = sqlx::query(
            r#"
            select id, email, role, created_at
            from users
            where id = $1
            order by created_at asc
            "#,
        )
        .bind(owner_user_id.into_uuid())
        .fetch_all(tx.as_mut())
        .await
        .map_err(db_err)?;
        tx.commit()
            .await
            .map_err(db_err)?;
        rows.into_iter().map(Self::map_user_info).collect()
    }

    async fn delete_user(
        &self,
        auth: &AuthContext,
        owner_user_id: UserId,
        user_id: Uuid,
    ) -> Result<(), AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        // Personal B2C: user must match the account owner id.
        let exists: bool = sqlx::query_scalar(
            "select exists(select 1 from users where id = $1 and id = $2)",
        )
        .bind(user_id)
        .bind(owner_user_id.into_uuid())
        .fetch_one(tx.as_mut())
        .await
        .map_err(db_err)?;
        if !exists {
            return Err(AppError::not_found(
                "user_not_found",
                "User not found for this account",
            ));
        }
        let deleted: i64 = sqlx::query_scalar("select delete_user_cascade($1)")
            .bind(user_id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(db_err)?;
        tx.commit()
            .await
            .map_err(db_err)?;
        if deleted > 0 {
            Ok(())
        } else {
            Err(AppError::not_found(
                "user_not_found",
                "User not found for this account",
            ))
        }
    }

    async fn set_account_blocked(
        &self,
        auth: &AuthContext,
        owner_user_id: UserId,
        blocked: bool,
    ) -> Result<(), AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let result = sqlx::query("update users set blocked = $2 where id = $1")
            .bind(owner_user_id.into_uuid())
            .bind(blocked)
            .execute(tx.as_mut())
            .await
            .map_err(db_err)?;
        if result.rows_affected() == 0 {
            return Err(AppError::not_found(
                "account_not_found",
                "Account not found",
            ));
        }
        tx.commit().await.map_err(db_err)?;
        Ok(())
    }
