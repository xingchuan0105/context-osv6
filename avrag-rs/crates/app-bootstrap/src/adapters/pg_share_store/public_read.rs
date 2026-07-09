    async fn load_shared_workspace(
        &self,
        token: &str,
    ) -> Result<Option<SharedWorkspaceSnapshot>, AppError> {
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
            select
              st.org_id,
              st.workspace_id,
              st.access_level,
              st.expires_at,
              n.allow_download
            from share_tokens st
            join workspaces n on n.id = st.workspace_id
            where st.token = $1
              and st.revoked_at is null
              and (st.expires_at is null or st.expires_at > now())
            "#,
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
        let org_id = row
            .try_get::<Uuid, _>("org_id")
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_org(tx.as_mut(), &org_id.to_string()).await?;
        let workspace_id = row
            .try_get::<Uuid, _>("workspace_id")
            .map_err(|error| AppError::internal(error.to_string()))?;
        let access_level = row
            .try_get::<String, _>("access_level")
            .map_err(|error| AppError::internal(error.to_string()))?;
        let expires_at = row
            .try_get::<Option<DateTime<Utc>>, _>("expires_at")
            .ok()
            .flatten();
        let allow_download = row.try_get::<bool, _>("allow_download").unwrap_or(false);
        sqlx::query("update share_tokens set access_count = access_count + 1 where token = $1")
            .bind(token)
            .execute(tx.as_mut())
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        sqlx::query(
            r#"
            insert into share_access_logs (org_id, workspace_id, share_token, action, created_at)
            values ($1, $2, $3, 'view', now())
            "#,
        )
        .bind(org_id)
        .bind(workspace_id)
        .bind(token)
        .execute(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        let notebook_row = sqlx::query("select title, description from workspaces where id = $1")
            .bind(workspace_id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        let title = notebook_row
            .try_get::<String, _>("title")
            .map_err(|error| AppError::internal(error.to_string()))?;
        let description = notebook_row.try_get::<String, _>("description").ok();
        let sources_rows = sqlx::query(
            r#"
            select id, file_name, status
            from documents
            where workspace_id = $1
              and status not in ('deleting', 'deleted')
            order by updated_at desc, created_at desc
            "#,
        )
        .bind(workspace_id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(Some(SharedWorkspaceSnapshot {
            knowledge_base: SharedKnowledgeBaseSnapshot {
                id: workspace_id.to_string(),
                title,
                description,
            },
            share: SharedShareInfoSnapshot {
                permission: ShareAccessLevel::from_role(&access_level)
                    .as_permission_label()
                    .to_string(),
                expires_at: expires_at.map(|dt| dt.to_rfc3339()),
                allow_download,
                scope: "full".to_string(),
            },
            sources: sources_rows
                .into_iter()
                .map(|row| SharedSourceSnapshot {
                    id: row
                        .try_get::<Uuid, _>("id")
                        .map(|id| id.to_string())
                        .unwrap_or_default(),
                    file_name: row.try_get("file_name").unwrap_or_default(),
                    status: row.try_get("status").unwrap_or_default(),
                })
                .collect(),
        }))
    }

    async fn resolve_public_share_chat_context(
        &self,
        token: &str,
    ) -> Result<Option<PublicShareChatContextSnapshot>, AppError> {
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
            select
              st.org_id,
              st.workspace_id,
              st.access_level,
              coalesce(n.owner_id, st.created_by) as owner_user_id
            from share_tokens st
            join workspaces n on n.id = st.workspace_id
            where st.token = $1
              and st.revoked_at is null
              and (st.expires_at is null or st.expires_at > now())
            "#,
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
        let org_id = row
            .try_get::<Uuid, _>("org_id")
            .map_err(|error| AppError::internal(error.to_string()))?;
        let workspace_id = row
            .try_get::<Uuid, _>("workspace_id")
            .map_err(|error| AppError::internal(error.to_string()))?;
        let access_level = row
            .try_get::<String, _>("access_level")
            .map_err(|error| AppError::internal(error.to_string()))?;
        let owner_user_id = row
            .try_get::<Option<Uuid>, _>("owner_user_id")
            .map_err(|error| AppError::internal(error.to_string()))?;
        let Some(owner_user_id) = owner_user_id else {
            tx.rollback()
                .await
                .map_err(|error| AppError::internal(error.to_string()))?;
            return Ok(None);
        };
        sqlx::query(
            r#"
            insert into share_access_logs (org_id, workspace_id, share_token, action, created_at)
            values ($1, $2, $3, 'chat', now())
            "#,
        )
        .bind(org_id)
        .bind(workspace_id)
        .bind(token)
        .execute(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(Some(PublicShareChatContextSnapshot {
            org_id,
            workspace_id,
            owner_user_id,
            access_level: ShareAccessLevel::from_role(&access_level),
        }))
    }
