use app_core::ChatPersistencePort;
use avrag_rag_core::context::SessionContext as RagSessionContext;
use contracts::chat::ChatMessage;
use contracts::workspaces::ChatSession;

use super::profile_merge::{apply_profile_delta, parse_profile_delta_response};
use super::profile_types::ProfileDelta;
use crate::context::ChatContext;

impl ChatContext {
    pub fn build_rag_session_context(messages: Vec<ChatMessage>) -> Option<RagSessionContext> {
        if messages.is_empty() {
            None
        } else {
            Some(RagSessionContext { messages })
        }
    }

    /// The "dream" layer: runs at most once per day per user.
    /// LLM proposes a semantic delta; runtime applies deterministic merge rules.
    pub(crate) async fn maybe_update_structured_profile(
        &self,
        chat_persistence: &dyn ChatPersistencePort,
        _session: &ChatSession,
        recent_turns: &str,
    ) -> bool {
        let Some(cm) = &self.orchestrator.chatmemory() else {
            return false;
        };
        let Some(user_id) = self.auth.actor_id().map(|value| value.into_uuid()) else {
            return false;
        };

        let existing_profile = chat_persistence
            .get_user_profile(&self.auth, user_id)
            .await
            .ok()
            .flatten();

        let should_dream = match &existing_profile {
            Some(profile) => {
                let since_last = chrono::Utc::now().signed_duration_since(profile.inferred_at);
                since_last.num_hours() >= 24
            }
            None => true,
        };
        if !should_dream {
            return false;
        }

        let existing_structured = existing_profile
            .as_ref()
            .map(|p| p.structured_profile.clone())
            .unwrap_or_else(|| serde_json::json!({}));

        let delta = self
            .infer_profile_delta(recent_turns, &existing_structured)
            .await;
        if delta.is_effectively_empty() {
            return false;
        }

        let merged = apply_profile_delta(existing_structured, delta);

        let expertise_domains = existing_profile
            .as_ref()
            .map(|p| p.expertise_domains.clone())
            .unwrap_or_default();
        let preferred_answer_style = existing_profile
            .as_ref()
            .and_then(|p| p.preferred_answer_style.clone());
        let frequently_asked_topics = existing_profile
            .as_ref()
            .map(|p| p.frequently_asked_topics.clone())
            .unwrap_or_default();
        let custom_preferences = existing_profile
            .as_ref()
            .map(|p| p.custom_preferences.clone())
            .unwrap_or_else(|| serde_json::json!({}));

        cm.update_user_profile(
            &self.auth,
            avrag_chatmemory::UserProfileUpdate {
                expertise_domains,
                preferred_answer_style,
                frequently_asked_topics,
                custom_preferences,
                structured_profile: merged,
                inference_version: "dream-v2".to_string(),
            },
        )
        .await
        .is_ok()
    }

    /// Ask the LLM to produce a semantic delta, not a full profile.
    pub(crate) async fn infer_profile_delta(
        &self,
        recent_turns: &str,
        existing_profile: &serde_json::Value,
    ) -> ProfileDelta {
        const DREAM_SYSTEM_PROMPT: &str =
            include_str!("../../../../prompts/pipeline/user-profile-extraction.system.md");
        let system_prompt = DREAM_SYSTEM_PROMPT.trim();
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let user_prompt = format!(
            "Today's date: {}\n\nExisting profile:\n{}\n\nRecent conversation turns:\n{}",
            today,
            serde_json::to_string_pretty(existing_profile).unwrap_or_else(|_| "{}".to_string()),
            recent_turns.trim()
        );

        for (llm, temperature) in [
            (
                self.llm_ctx.memory_client(),
                self.llm_ctx.memory_llm_temperature(),
            ),
            (
                self.llm_ctx.agent_client(),
                self.llm_ctx.agent_llm_temperature(),
            ),
        ] {
            if let Some(client) = llm
                && let Ok(response) = client
                    .complete(
                        &[
                            avrag_llm::ChatMessage::system(system_prompt),
                            avrag_llm::ChatMessage::user(&user_prompt),
                        ],
                        temperature,
                    )
                    .await
            {
                let trimmed = response.content.trim();
                if !trimmed.is_empty() {
                    let (sanitized, guard_report) = self
                        .orchestrator
                        .guard_pipeline()
                        .check_output(trimmed, None);
                    if guard_report.blocked {
                        tracing::warn!(
                            "dream layer output blocked by guard pipeline: {:?}",
                            guard_report.degrade_trace
                        );
                        return ProfileDelta::default();
                    }
                    self.record_llm_usage_if_available(
                        avrag_billing::usage_limit::BillableFeature::Summary,
                        "dream_delta",
                        &response.usage,
                        "inline",
                    )
                    .await;
                    return parse_profile_delta_response(&sanitized);
                }
            }
        }
        ProfileDelta::default()
    }
}
