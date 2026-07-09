//! Bound face — prefs.

use app_core::StorageContext;
use contracts::auth_runtime::AuthContext;
use contracts::preferences::AgentPreferenceMemory;
use contracts::UserPreferences;
use uuid::Uuid;


pub struct BoundPrefs<'a> {
    pub(crate) admin: &'a app_admin::AdminContext,
    pub(crate) auth: &'a AuthContext,
    pub(crate) storage: &'a StorageContext,
}

impl<'a> BoundPrefs<'a> {
    pub async fn load(&self, user_id: Uuid) -> Result<UserPreferences, common::AppError> {
        self.admin
            .load_user_preferences(self.auth, self.storage, user_id)
            .await
    }

    pub async fn save(
        &self,
        user_id: Uuid,
        preferences: &UserPreferences,
    ) -> Result<UserPreferences, common::AppError> {
        self.admin
            .save_user_preferences(self.auth, self.storage, user_id, preferences)
            .await
    }

    pub async fn current(&self) -> Result<UserPreferences, common::AppError> {
        self.admin
            .current_user_preferences(self.auth, self.storage)
            .await
    }

    pub async fn save_current(
        &self,
        preferences: &UserPreferences,
    ) -> Result<UserPreferences, common::AppError> {
        self.admin
            .save_current_user_preferences(self.auth, self.storage, preferences)
            .await
    }

    pub async fn delete_current_agent_preference(
        &self,
        preference_id: &str,
    ) -> Result<Option<AgentPreferenceMemory>, common::AppError> {
        self.admin
            .delete_current_agent_preference(self.auth, self.storage, preference_id)
            .await
    }

    pub async fn remember_explicit_agent_preference(
        &self,
        query: &str,
    ) -> Result<(), common::AppError> {
        self.admin
            .remember_explicit_agent_preference(self.auth, self.storage, query)
            .await
    }
}
