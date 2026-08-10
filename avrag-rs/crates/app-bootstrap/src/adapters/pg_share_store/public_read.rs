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
              st.owner_user_id,
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
        let owner_user_id = row
            .try_get::<Uuid, _>("owner_user_id")
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_rls_owner(tx.as_mut(), &owner_user_id.to_string()).await?;
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
            insert into share_access_logs (owner_user_id, workspace_id, share_token, action, created_at)
            values ($1, $2, $3, 'view', now())
            "#,
        )
        .bind(owner_user_id)
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
        let owner_row = sqlx::query(
            r#"
            select full_name, email, bio, contact_url, avatar_object_path, banner_object_path, public_profile_enabled
            from users
            where id = $1
            "#,
        )
        .bind(owner_user_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        let owner = owner_row.map(|row| {
            let full_name = row
                .try_get::<Option<String>, _>("full_name")
                .ok()
                .flatten()
                .filter(|s| !s.trim().is_empty());
            let email = row.try_get::<String, _>("email").unwrap_or_default();
            let display_name = full_name.unwrap_or_else(|| {
                email
                    .split('@')
                    .next()
                    .unwrap_or("Owner")
                    .to_string()
            });
            let bio = row
                .try_get::<Option<String>, _>("bio")
                .ok()
                .flatten()
                .filter(|s| !s.trim().is_empty());
            let contact_url = row
                .try_get::<Option<String>, _>("contact_url")
                .ok()
                .flatten()
                .filter(|s| !s.trim().is_empty());
            let avatar_path = row
                .try_get::<Option<String>, _>("avatar_object_path")
                .ok()
                .flatten()
                .filter(|s| !s.trim().is_empty());
            let banner_path = row
                .try_get::<Option<String>, _>("banner_object_path")
                .ok()
                .flatten()
                .filter(|s| !s.trim().is_empty());
            let profile_enabled = row
                .try_get::<bool, _>("public_profile_enabled")
                .unwrap_or(false);
            ShareOwnerCardSnapshot {
                user_id: Some(owner_user_id.to_string()),
                display_name,
                bio,
                contact_url,
                avatar_url: avatar_path.map(|_| {
                    format!("/api/public/users/{owner_user_id}/media/avatar")
                }),
                banner_url: banner_path.map(|_| {
                    format!("/api/public/users/{owner_user_id}/media/banner")
                }),
                profile_enabled,
            }
        });
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
            owner,
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
              coalesce(n.owner_id, st.owner_user_id, st.created_by) as owner_user_id,
              st.workspace_id,
              st.access_level as token_access_level,
              coalesce(n.access_level, 'private') as workspace_visibility,
              coalesce(n.share_enabled, false) as share_enabled,
              coalesce(n.share_anon_question_limit, 10) as share_anon_question_limit,
              n.share_member_question_limit
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
        let owner_user_id = row
            .try_get::<Option<Uuid>, _>("owner_user_id")
            .map_err(|error| AppError::internal(error.to_string()))?;
        let Some(owner_user_id) = owner_user_id else {
            tx.rollback()
                .await
                .map_err(|error| AppError::internal(error.to_string()))?;
            return Ok(None);
        };
        let workspace_id = row
            .try_get::<Uuid, _>("workspace_id")
            .map_err(|error| AppError::internal(error.to_string()))?;
        let access_level = row
            .try_get::<String, _>("token_access_level")
            .map_err(|error| AppError::internal(error.to_string()))?;
        let workspace_visibility = row
            .try_get::<String, _>("workspace_visibility")
            .map_err(|error| AppError::internal(error.to_string()))?;
        let share_enabled = row
            .try_get::<bool, _>("share_enabled")
            .map_err(|error| AppError::internal(error.to_string()))?;
        let anon_question_limit = row
            .try_get::<i32, _>("share_anon_question_limit")
            .unwrap_or(10);
        let member_question_limit = row
            .try_get::<Option<i32>, _>("share_member_question_limit")
            .ok()
            .flatten();
        sqlx::query(
            r#"
            insert into share_access_logs (owner_user_id, workspace_id, share_token, action, created_at)
            values ($1, $2, $3, 'chat', now())
            "#,
        )
        .bind(owner_user_id)
        .bind(workspace_id)
        .bind(token)
        .execute(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(Some(PublicShareChatContextSnapshot {
            owner_user_id,
            workspace_id,
            access_level: ShareAccessLevel::from_role(&access_level),
            workspace_visibility,
            share_enabled,
            anon_question_limit,
            member_question_limit,
        }))
    }

    async fn list_public_shares_for_owner(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<PublicOwnerShareItemSnapshot>, AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_role(tx.as_mut(), "super_admin").await?;
        set_rls_owner(tx.as_mut(), &user_id.to_string()).await?;
        let rows = sqlx::query(
            r#"
            select
              st.workspace_id,
              st.token,
              st.access_level,
              w.title,
              w.description,
              w.allow_download,
              (select count(*) from documents d
               where d.workspace_id = st.workspace_id
                 and d.status not in ('deleting', 'deleted')) as source_count
            from share_tokens st
            join workspaces w on w.id = st.workspace_id
            where (st.owner_user_id = $1 or w.owner_id = $1)
              and st.revoked_at is null
              and (st.expires_at is null or st.expires_at > now())
            order by st.created_at desc
            "#,
        )
        .bind(user_id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        // One row per workspace: an owner may hold several active tokens on the
        // same workspace; the latest (first after `order by created_at desc`) wins.
        let mut seen = std::collections::HashSet::new();
        let items = rows
            .into_iter()
            .filter_map(|row| {
                let workspace_id = row.try_get::<Uuid, _>("workspace_id").ok()?;
                if !seen.insert(workspace_id) {
                    return None;
                }
                let access_level = row
                    .try_get::<String, _>("access_level")
                    .unwrap_or_default();
                Some(PublicOwnerShareItemSnapshot {
                    workspace_id: workspace_id.to_string(),
                    title: row.try_get("title").unwrap_or_default(),
                    description: row.try_get("description").ok(),
                    share_token: row.try_get("token").unwrap_or_default(),
                    access_level: ShareAccessLevel::from_role(&access_level)
                        .as_permission_label()
                        .to_string(),
                    allow_download: row.try_get("allow_download").unwrap_or(false),
                    source_count: row.try_get("source_count").unwrap_or(0),
                })
            })
            .collect();
        Ok(items)
    }
