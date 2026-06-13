use serde::{Deserialize, Serialize};

/// LLM-produced semantic delta for the dream layer; runtime applies deterministic merge.
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
            && self.preferred_answer_style_update.as_ref().is_none_or(|u| u.is_noop())
            && self.preferred_language_update.as_ref().is_none_or(|u| u.is_noop())
    }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SlotUpdate {
    pub tag: Option<String>,
    pub action: Option<String>,
    pub description: Option<String>,
    pub reason: Option<String>,
    pub evidence: Vec<serde_json::Value>,
    pub confidence_signal: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SingletonUpdate {
    pub tag: Option<String>,
    pub action: Option<String>,
    pub description: Option<String>,
    pub evidence: Vec<serde_json::Value>,
    pub confidence_signal: Option<String>,
    pub value: Option<serde_json::Value>,
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
    pub evidence: Vec<serde_json::Value>,
}
