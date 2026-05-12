use common::{
    AppError, new_id,
    now_rfc3339,
};
use contracts::UserPreferences;
use uuid::Uuid;

use crate::lib_impl::*;

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

    pub async fn delete_current_agent_preference(
        &self,
        preference_id: &str,
    ) -> Result<Option<common::AgentPreferenceMemory>, AppError> {
        let mut preferences = self.current_user_preferences().await?;
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
            preferences.agent_memory.blocked.push(common::BlockedAgentPreference {
                id: removed.id,
                text: removed.text,
                blocked_at: now_rfc3339(),
            });
        }

        let saved = self.save_current_user_preferences(&preferences).await?;
        Ok(Some(saved.agent_memory))
    }

    pub(crate) async fn remember_explicit_agent_preference(
        &self,
        query: &str,
    ) -> Result<(), AppError> {
        let Some(text) = explicit_agent_preference_text(query) else {
            return Ok(());
        };

        let mut preferences = match self.current_user_preferences().await {
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
            preferences.agent_memory.active.push(common::AgentPreference {
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
                preferences.agent_memory.daily_log.push(common::DailyPreferenceLog {
                    date,
                    added: vec![normalized],
                    no_change: Vec::new(),
                });
            }
        }

        let _ = self.save_current_user_preferences(&preferences).await?;
        Ok(())
    }
}

fn remove_agent_preference(
    memory: &mut common::AgentPreferenceMemory,
    preference_id: &str,
) -> Option<common::AgentPreference> {
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
