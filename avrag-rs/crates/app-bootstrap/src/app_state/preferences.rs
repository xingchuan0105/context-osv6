use common::AppError;
use contracts::UserPreferences;
use contracts::preferences::AgentPreferenceMemory;
use uuid::Uuid;

use super::AppState;

impl AppState {
    pub async fn load_user_preferences(&self, user_id: Uuid) -> Result<UserPreferences, AppError> {
        self.admin
            .load_user_preferences(&self.auth, &self.storage, user_id)
            .await
    }

    pub async fn save_user_preferences(
        &self,
        user_id: Uuid,
        preferences: &UserPreferences,
    ) -> Result<UserPreferences, AppError> {
        self.admin
            .save_user_preferences(&self.auth, &self.storage, user_id, preferences)
            .await
    }

    pub async fn current_user_preferences(&self) -> Result<UserPreferences, AppError> {
        self.admin
            .current_user_preferences(&self.auth, &self.storage)
            .await
    }

    pub async fn save_current_user_preferences(
        &self,
        preferences: &UserPreferences,
    ) -> Result<UserPreferences, AppError> {
        self.admin
            .save_current_user_preferences(&self.auth, &self.storage, preferences)
            .await
    }

    pub async fn delete_current_agent_preference(
        &self,
        preference_id: &str,
    ) -> Result<Option<AgentPreferenceMemory>, AppError> {
        self.admin
            .delete_current_agent_preference(&self.auth, &self.storage, preference_id)
            .await
    }

    pub async fn remember_explicit_agent_preference(&self, query: &str) -> Result<(), AppError> {
        self.admin
            .remember_explicit_agent_preference(&self.auth, &self.storage, query)
            .await
    }
}
