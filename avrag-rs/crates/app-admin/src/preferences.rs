use app_core::{domain_rows::UserProfileRow, StorageContext};
use avrag_auth::AuthContext;
use common::{new_id, now_rfc3339, AppError};
use contracts::preferences::{AgentPreference, AgentPreferenceMemory, BlockedAgentPreference, DailyPreferenceLog};
use contracts::UserPreferences;
use uuid::Uuid;

use crate::AdminContext;

impl AdminContext {
    pub async fn load_user_preferences(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        user_id: Uuid,
    ) -> Result<UserPreferences, AppError> {
        if let Some(store) = storage.admin_store() {
            let profile = store.get_user_profile(auth, user_id).await?;
            let preferences = profile
                .and_then(|row| {
                    serde_json::from_value::<UserPreferences>(row.custom_preferences).ok()
                })
                .unwrap_or_default();
            return Ok(preferences);
        }

        let state = storage.inner().read().await;
        Ok(state
            .user_preferences
            .get(&user_id.to_string())
            .cloned()
            .unwrap_or_default())
    }

    pub async fn save_user_preferences(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        user_id: Uuid,
        preferences: &UserPreferences,
    ) -> Result<UserPreferences, AppError> {
        if let Some(store) = storage.admin_store() {
            let existing_profile = store.get_user_profile(auth, user_id).await?;
            let profile = UserProfileRow {
                user_id,
                org_id: auth.org_id(),
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
                structured_profile: existing_profile
                    .as_ref()
                    .map(|profile| profile.structured_profile.clone())
                    .unwrap_or_else(|| serde_json::json!({})),
                inferred_at: chrono::Utc::now(),
                inference_version: existing_profile
                    .as_ref()
                    .map(|profile| profile.inference_version.clone())
                    .unwrap_or_else(|| "preferences-v1".to_string()),
            };
            store.upsert_user_profile(auth, &profile).await?;
            return Ok(preferences.clone());
        }

        let mut state = storage.inner().write().await;
        state
            .user_preferences
            .insert(user_id.to_string(), preferences.clone());
        Ok(preferences.clone())
    }

    pub async fn current_user_preferences(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
    ) -> Result<UserPreferences, AppError> {
        let user_id = auth
            .actor_id()
            .map(|value| value.into_uuid())
            .ok_or_else(|| {
                AppError::unauthorized("user preferences require an authenticated user")
            })?;
        self.load_user_preferences(auth, storage, user_id).await
    }

    pub async fn save_current_user_preferences(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        preferences: &UserPreferences,
    ) -> Result<UserPreferences, AppError> {
        let user_id = auth
            .actor_id()
            .map(|value| value.into_uuid())
            .ok_or_else(|| {
                AppError::unauthorized("user preferences require an authenticated user")
            })?;
        self.save_user_preferences(auth, storage, user_id, preferences)
            .await
    }

    pub async fn delete_current_agent_preference(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        preference_id: &str,
    ) -> Result<Option<AgentPreferenceMemory>, AppError> {
        let mut preferences = self.current_user_preferences(auth, storage).await?;
        let removed = remove_agent_preference(&mut preferences.agent_memory, preference_id);
        let Some(removed) = removed else {
            return Ok(None);
        };

        if !preferences
            .agent_memory
            .blocked
            .iter()
            .any(|blocked| blocked.id == removed.id)
        {
            preferences.agent_memory.blocked.push(BlockedAgentPreference {
                id: removed.id,
                text: removed.text,
                blocked_at: now_rfc3339(),
            });
        }

        let saved = self
            .save_current_user_preferences(auth, storage, &preferences)
            .await?;
        Ok(Some(saved.agent_memory))
    }

    pub async fn remember_explicit_agent_preference(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        query: &str,
    ) -> Result<(), AppError> {
        let Some(text) = explicit_agent_preference_text(query) else {
            return Ok(());
        };

        let mut preferences = match self.current_user_preferences(auth, storage).await {
            Ok(preferences) => preferences,
            Err(_) => return Ok(()),
        };
        let normalized = normalize_preference_text(&text);
        if normalized.is_empty()
            || preferences
                .agent_memory
                .blocked
                .iter()
                .any(|blocked| normalize_preference_text(&blocked.text) == normalized)
        {
            return Ok(());
        }

        let now = now_rfc3339();
        if let Some(existing) = preferences
            .agent_memory
            .active
            .iter_mut()
            .find(|preference| normalize_preference_text(&preference.text) == normalized)
        {
            existing.updated_at = now;
        } else {
            preferences.agent_memory.active.push(AgentPreference {
                id: new_id(),
                text,
                category: "interaction".to_string(),
                scope: "global".to_string(),
                confidence: "explicit".to_string(),
                source: "explicit_user_turn".to_string(),
                updated_at: now.clone(),
            });
            let date = now.split('T').next().unwrap_or(&now).to_string();
            if let Some(log) = preferences
                .agent_memory
                .daily_log
                .iter_mut()
                .find(|log| log.date == date)
            {
                log.added.push(normalized);
            } else {
                preferences.agent_memory.daily_log.push(DailyPreferenceLog {
                    date,
                    added: vec![normalized],
                    no_change: Vec::new(),
                });
            }
        }

        let _ = self
            .save_current_user_preferences(auth, storage, &preferences)
            .await?;
        Ok(())
    }
}

fn remove_agent_preference(
    memory: &mut AgentPreferenceMemory,
    preference_id: &str,
) -> Option<AgentPreference> {
    if let Some(index) = memory
        .active
        .iter()
        .position(|preference| preference.id == preference_id)
    {
        return Some(memory.active.remove(index));
    }
    memory
        .superseded
        .iter()
        .position(|preference| preference.id == preference_id)
        .map(|index| memory.superseded.remove(index))
}

fn explicit_agent_preference_text(query: &str) -> Option<String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }

    for prefix in ["请记住", "记住", "以后都", "以后请"] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let text = rest.trim_matches([' ', ':', '：', ',', '，', '.', '。']);
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }

    let lower = trimmed.to_ascii_lowercase();
    for prefix in ["remember that ", "remember "] {
        if lower.starts_with(prefix) {
            let text = trimmed[prefix.len()..].trim_matches([' ', ':', ',', '.', ';']);
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }

    None
}

fn normalize_preference_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}
