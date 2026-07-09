    async fn get_user_profile(
        &self,
        auth: &AuthContext,
        user_id: Uuid,
    ) -> Result<Option<UserProfileRow>, AppError> {
        self.repo
            .auth().get_user_profile(auth, user_id)
            .await
            .map_err(map_pg_error)
            .map(|profile| profile.map(user_profile_row))
    }

    async fn upsert_user_profile(
        &self,
        auth: &AuthContext,
        profile: &UserProfileRow,
    ) -> Result<(), AppError> {
        self.repo
            .auth().upsert_user_profile(auth, &user_profile_row_to_pg(profile))
            .await
            .map_err(map_pg_error)
    }

    async fn list_api_keys(
        &self,
        auth: &AuthContext,
        workspace_id: Option<Uuid>,
    ) -> Result<Vec<ApiKeyRow>, AppError> {
        self.repo
            .auth().list_api_keys(auth, workspace_id)
            .await
            .map_err(map_pg_error)
    }

    async fn create_api_key(
        &self,
        auth: &AuthContext,
        workspace_id: Option<Uuid>,
        name: &str,
        permissions: &[String],
        rate_limit_rpm: i32,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<CreateApiKeyResponse, AppError> {
        let (api_key, plaintext_key) = self
            .repo
            .auth().create_api_key(
                auth,
                workspace_id,
                name,
                permissions,
                rate_limit_rpm.max(0) as u32,
                expires_at,
            )
            .await
            .map_err(map_pg_error)?;
        Ok(CreateApiKeyResponse {
            api_key,
            plaintext_key,
        })
    }

    async fn revoke_api_key(
        &self,
        auth: &AuthContext,
        workspace_id: Option<Uuid>,
        key_id: Uuid,
    ) -> Result<bool, AppError> {
        self.repo
            .auth().revoke_api_key(auth, workspace_id, key_id)
            .await
            .map_err(map_pg_error)
    }

    async fn list_notifications(
        &self,
        auth: &AuthContext,
        user_id: Uuid,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<NotificationRow>, AppError> {
        self.repo
            .auth().list_notifications(auth, user_id, limit, offset)
            .await
            .map_err(map_pg_error)
    }

    async fn mark_notification_read(
        &self,
        auth: &AuthContext,
        user_id: Uuid,
        notification_id: Uuid,
    ) -> Result<bool, AppError> {
        self.repo
            .auth().mark_notification_read(auth, user_id, notification_id)
            .await
            .map_err(map_pg_error)
    }

