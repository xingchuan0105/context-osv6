use serde::{Deserialize, Serialize};

/// Canonical stored profile (Chat + WebSearch only).
/// Persistence boundary: encode/decode via `user_profile_to_value` / `user_profile_from_value`.
#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct UserProfile {
    pub expertise_domains: Vec<ProfileSlot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_answer_style: Option<ProfileSingleton>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_language: Option<ProfileSingleton>,
    pub tool_preferences: Vec<ProfileSlot>,
    pub important_constraints: Vec<ProfileSlot>,
    pub session_continuity_hints: Vec<StoredSessionHint>,
    pub observed_conflicts: Vec<StoredConflict>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_summary: Option<String>,
}

/// One expertise / tool / constraint slot in stored profile.
#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct ProfileSlot {
    pub tag: Option<String>,
    pub action: Option<String>,
    pub description: Option<String>,
    pub reason: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_list")]
    pub evidence: Vec<String>,
    pub confidence_signal: Option<String>,
    pub expires_at: Option<String>,
    pub confidence: Option<f64>,
    pub since: Option<String>,
    pub last_seen: Option<String>,
}

/// Preferred language / answer-style singleton in stored profile.
#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct ProfileSingleton {
    pub tag: Option<String>,
    pub action: Option<String>,
    pub description: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_list")]
    pub evidence: Vec<String>,
    pub confidence_signal: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string")]
    pub value: Option<String>,
    pub confidence: Option<f64>,
    pub since: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct StoredSessionHint {
    pub hint: Option<String>,
    pub source_session_id: Option<String>,
    pub priority: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct StoredConflict {
    pub field: Option<String>,
    pub old_view: Option<String>,
    pub new_view: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_list")]
    pub evidence: Vec<String>,
}

/// LLM-produced semantic delta for the dream layer; runtime applies deterministic merge.
///
/// Scope: Chat + WebSearch experience only (see PROFILE_MEMORY_SCOPE_CHAT_SEARCH.md).
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ProfileDelta {
    pub expertise_domain_updates: Vec<SlotUpdate>,
    pub preferred_answer_style_update: Option<SingletonUpdate>,
    pub preferred_language_update: Option<SingletonUpdate>,
    pub tool_preference_updates: Vec<SlotUpdate>,
    pub important_constraint_updates: Vec<SlotUpdate>,
    pub session_continuity_hints: Vec<SessionHint>,
    pub observed_conflicts: Vec<ObservedConflict>,
    pub global_summary: Option<String>,
}

impl ProfileDelta {
    pub fn is_effectively_empty(&self) -> bool {
        self.expertise_domain_updates.is_empty()
            && self.tool_preference_updates.is_empty()
            && self.important_constraint_updates.is_empty()
            && self.session_continuity_hints.is_empty()
            && self.observed_conflicts.is_empty()
            && self.global_summary.as_ref().is_none_or(|s| s.is_empty())
            && self
                .preferred_answer_style_update
                .as_ref()
                .is_none_or(|u| u.is_noop())
            && self
                .preferred_language_update
                .as_ref()
                .is_none_or(|u| u.is_noop())
    }
}

/// Short evidence snippets (plain text) supporting a profile slot update.
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SlotUpdate {
    pub tag: Option<String>,
    pub action: Option<String>,
    pub description: Option<String>,
    pub reason: Option<String>,
    /// Typed evidence lines (LLM may historically send free JSON; serde coerces strings).
    #[serde(deserialize_with = "deserialize_string_list")]
    pub evidence: Vec<String>,
    pub confidence_signal: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SingletonUpdate {
    pub tag: Option<String>,
    pub action: Option<String>,
    pub description: Option<String>,
    #[serde(deserialize_with = "deserialize_string_list")]
    pub evidence: Vec<String>,
    pub confidence_signal: Option<String>,
    /// Preferred style / language tag as plain string when present.
    #[serde(default, deserialize_with = "deserialize_opt_string")]
    pub value: Option<String>,
}

impl SingletonUpdate {
    pub fn is_noop(&self) -> bool {
        self.action.as_deref() == Some("none") || self.action.is_none()
    }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SessionHint {
    pub hint: Option<String>,
    pub source_session_id: Option<String>,
    pub priority: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ObservedConflict {
    pub field: Option<String>,
    pub old_view: Option<String>,
    pub new_view: Option<String>,
    #[serde(deserialize_with = "deserialize_string_list")]
    pub evidence: Vec<String>,
}

fn deserialize_string_list<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(match value {
        serde_json::Value::Null => vec![],
        serde_json::Value::Array(items) => items
            .into_iter()
            .filter_map(|item| match item {
                serde_json::Value::String(s) => Some(s),
                other => Some(other.to_string()),
            })
            .collect(),
        serde_json::Value::String(s) => vec![s],
        other => vec![other.to_string()],
    })
}

fn deserialize_opt_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(match value {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(s)) => Some(s),
        Some(other) => Some(other.to_string()),
    })
}
