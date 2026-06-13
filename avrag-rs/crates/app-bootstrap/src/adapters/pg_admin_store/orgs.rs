    async fn list_orgs(
        &self,
        auth: &AuthContext,
        page: usize,
        per_page: usize,
    ) -> Result<Vec<AdminOrgInfo>, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let page = page.max(1);
        let per_page = admin_clamp_org_list_per_page(per_page);
        let offset = ((page - 1) * per_page) as i64;
        let rows = sqlx::query(
            r#"
            select
              o.id,
              o.name,
              o.created_at,
              o.blocked,
              count(distinct u.id) as user_count,
              count(distinct d.id) as document_count,
              count(distinct m.id) filter (where m.role = 'user') as query_count
            from organizations o
            left join users u on u.org_id = o.id
            left join documents d on d.org_id = o.id
            left join chat_messages m on m.org_id = o.id
            group by o.id, o.name, o.created_at, o.blocked
            order by o.created_at desc
            limit $1 offset $2
            "#,
        )
        .bind(per_page as i64)
        .bind(offset)
        .fetch_all(tx.as_mut())
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        rows.into_iter().map(Self::map_org_info).collect()
    }

    async fn get_org(&self, auth: &AuthContext, org_id: OrgId) -> Result<AdminOrgInfo, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let row = sqlx::query(
            r#"
            select
              o.id,
              o.name,
              o.created_at,
              o.blocked,
              (select count(*) from users u where u.org_id = o.id) as user_count,
              (select count(*) from documents d where d.org_id = o.id) as document_count,
              (select count(*) from chat_messages m where m.org_id = o.id and m.role = 'user') as query_count
            from organizations o
            where o.id = $1
            "#,
        )
        .bind(org_id.into_uuid())
        .fetch_optional(tx.as_mut())
        .await
        .map_err(db_err)?;
        tx.commit()
            .await
            .map_err(db_err)?;
        row.map(Self::map_org_info)
            .transpose()?
            .ok_or_else(|| AppError::not_found("org_not_found", "Organization not found"))
    }

    async fn list_users(
        &self,
        auth: &AuthContext,
        org_id: OrgId,
    ) -> Result<Vec<AdminUserInfo>, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let rows = sqlx::query(
            r#"
            select id, email, org_id, role, created_at
            from users
            where org_id = $1
            order by created_at asc
            "#,
        )
        .bind(org_id.into_uuid())
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
        org_id: OrgId,
        user_id: Uuid,
    ) -> Result<(), AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let exists: bool = sqlx::query_scalar(
            "select exists(select 1 from users where id = $1 and org_id = $2)",
        )
        .bind(user_id)
        .bind(org_id.into_uuid())
        .fetch_one(tx.as_mut())
        .await
        .map_err(db_err)?;
        if !exists {
            return Err(AppError::not_found(
                "user_not_found",
                "User not found in this organization",
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
                "User not found in this organization",
            ))
        }
    }

    async fn set_org_blocked(
        &self,
        auth: &AuthContext,
        org_id: OrgId,
        blocked: bool,
    ) -> Result<(), AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let result = sqlx::query("update organizations set blocked = $2 where id = $1")
            .bind(org_id.into_uuid())
            .bind(blocked)
            .execute(tx.as_mut())
            .await
            .map_err(db_err)?;
        if result.rows_affected() == 0 {
            return Err(AppError::not_found(
                "org_not_found",
                "Organization not found",
            ));
        }
        tx.commit().await.map_err(db_err)?;
        Ok(())
    }

