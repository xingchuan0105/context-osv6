impl AppState {
    pub async fn load_user_preferences(
        &self,
        user_id: Uuid,
    ) -> Result<UserPreferences, AppError> {
        if let Some(pg) = &self.pg {
            let profile = pg
                .get_user_profile(&self.auth, user_id)
                .await
                .map_err(map_pg_error)?;
            let preferences = profile
                .and_then(|row| serde_json::from_value::<UserPreferences>(row.custom_preferences).ok())
                .unwrap_or_default();
            return Ok(preferences);
        }

        let state = self.inner.read().await;
        Ok(state
            .user_preferences
            .get(&user_id.to_string())
            .cloned()
            .unwrap_or_default())
    }

    pub async fn save_user_preferences(
        &self,
        user_id: Uuid,
        preferences: &UserPreferences,
    ) -> Result<UserPreferences, AppError> {
        if let Some(pg) = &self.pg {
            let existing_profile = pg
                .get_user_profile(&self.auth, user_id)
                .await
                .map_err(map_pg_error)?;
            let profile = avrag_storage_pg::UserProfileRow {
                user_id,
                org_id: self.auth.org_id(),
                expertise_domains: existing_profile
                    .as_ref()
                    .map(|profile| profile.expertise_domains.clone())
                    .unwrap_or_default(),
                preferred_answer_style: existing_profile
                    .as_ref()
                    .and_then(|profile| profile.preferred_answer_style.clone()),
                frequently_asked_topics: existing_profile
                    .as_ref()
                    .map(|profile| profile.frequently_asked_topics.clone())
                    .unwrap_or_default(),
                custom_preferences: serde_json::to_value(preferences)
                    .unwrap_or_else(|_| serde_json::json!({})),
                inferred_at: chrono::Utc::now(),
                inference_version: existing_profile
                    .as_ref()
                    .map(|profile| profile.inference_version.clone())
                    .unwrap_or_else(|| "preferences-v1".to_string()),
            };
            pg.upsert_user_profile(&self.auth, &profile)
                .await
                .map_err(map_pg_error)?;
            return Ok(preferences.clone());
        }

        let mut state = self.inner.write().await;
        state
            .user_preferences
            .insert(user_id.to_string(), preferences.clone());
        Ok(preferences.clone())
    }

    pub async fn current_user_preferences(&self) -> Result<UserPreferences, AppError> {
        let user_id = self
            .auth
            .actor_id()
            .map(|value| value.into_uuid())
            .ok_or_else(|| {
                AppError::unauthorized("user preferences require an authenticated user")
            })?;
        self.load_user_preferences(user_id).await
    }

    pub async fn save_current_user_preferences(
        &self,
        preferences: &UserPreferences,
    ) -> Result<UserPreferences, AppError> {
        let user_id = self
            .auth
            .actor_id()
            .map(|value| value.into_uuid())
            .ok_or_else(|| {
                AppError::unauthorized("user preferences require an authenticated user")
            })?;
        self.save_user_preferences(user_id, preferences).await
    }
}
