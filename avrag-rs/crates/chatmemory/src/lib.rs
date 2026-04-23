use avrag_auth::AuthContext;
use avrag_storage_pg::{DialogueStateRow, PgAppRepository, UserProfileRow};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use uuid::Uuid;

pub mod v1;

pub use v1::{
    LongTermMemoryStore, MemoryObject, NoopMemvidStore, WorkspaceLongTermMemory,
    WorkspaceMemoryTurn, WorkspaceShortTermMemoryWindow, build_short_term_memory_window,
    dedupe_adjacent_assistant_messages, should_refresh_long_term_memory,
};

/// Layer 1: Raw messages (last N messages)
pub struct Layer1Messages {
    pub messages: Vec<common::ChatMessage>,
}

/// Layer 2: Session summary
pub struct Layer2Summary {
    pub session_id: Uuid,
    pub summary: String,
    pub updated_at: DateTime<Utc>,
}

/// Layer 3: User profile across sessions
pub struct Layer3Profile {
    pub user_id: Uuid,
    pub expertise_domains: Vec<String>,
    pub preferred_answer_style: Option<String>,
    pub frequently_asked_topics: Vec<String>,
    pub custom_preferences: serde_json::Value,
    pub inferred_at: DateTime<Utc>,
    pub inference_version: String,
}

/// Working memory for the current dialogue/session
pub struct WorkingMemory {
    pub session_id: Uuid,
    pub state_type: String,
    pub current_topic: Option<String>,
    pub pending_questions: Vec<String>,
    pub gathered_facts: Vec<String>,
    pub confidence_score: f32,
    pub state_history: Vec<String>,
    pub last_updated_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct ChatMemory {
    repo: Arc<PgAppRepository>,
}

impl ChatMemory {
    pub fn new(repo: Arc<PgAppRepository>) -> Self {
        Self { repo }
    }

    pub async fn load(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
    ) -> anyhow::Result<ChatMemoryData> {
        let messages = self.repo.list_messages(auth, session_id).await?;
        let session = self.repo.get_session(auth, session_id).await?;
        let summary = session.and_then(|s| s.summary);

        let actor_id = auth.actor_id().map(|value| value.into_uuid());
        let profile = if let Some(user_id) = actor_id {
            self.repo
                .get_user_profile(auth, user_id)
                .await?
                .map(map_profile)
        } else {
            None
        };
        let working_memory = self
            .repo
            .get_dialogue_state(auth, session_id)
            .await?
            .map(map_working_memory);

        Ok(ChatMemoryData {
            layer1: Layer1Messages { messages },
            layer2: summary.map(|value| Layer2Summary {
                session_id,
                summary: value,
                updated_at: Utc::now(),
            }),
            layer3: profile,
            working_memory,
        })
    }

    pub async fn update_summary(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
        new_summary: &str,
    ) -> anyhow::Result<()> {
        self.repo
            .update_session_summary(auth, session_id, new_summary)
            .await?;
        Ok(())
    }

    pub async fn update_user_profile(
        &self,
        auth: &AuthContext,
        expertise_domains: Vec<String>,
        preferred_answer_style: Option<String>,
        frequently_asked_topics: Vec<String>,
        custom_preferences: serde_json::Value,
        inference_version: &str,
    ) -> anyhow::Result<()> {
        let Some(user_id) = auth.actor_id().map(|value| value.into_uuid()) else {
            return Ok(());
        };
        let profile = UserProfileRow {
            user_id,
            org_id: auth.org_id(),
            expertise_domains,
            preferred_answer_style,
            frequently_asked_topics,
            custom_preferences,
            inferred_at: Utc::now(),
            inference_version: inference_version.to_string(),
        };
        self.repo.upsert_user_profile(auth, &profile).await?;
        Ok(())
    }

    pub async fn update_working_memory(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
        state_type: &str,
        current_topic: Option<String>,
        pending_questions: Vec<String>,
        gathered_facts: Vec<String>,
        confidence_score: f32,
        state_history: Vec<String>,
    ) -> anyhow::Result<()> {
        let state = DialogueStateRow {
            id: Uuid::new_v4(),
            org_id: auth.org_id(),
            session_id,
            user_id: auth.actor_id().map(|value| value.into_uuid()),
            state_type: state_type.to_string(),
            current_topic,
            pending_questions,
            gathered_facts,
            confidence_score,
            state_history,
            last_updated_at: Utc::now(),
        };
        self.repo.upsert_dialogue_state(auth, &state).await?;
        Ok(())
    }
}

pub struct ChatMemoryData {
    pub layer1: Layer1Messages,
    pub layer2: Option<Layer2Summary>,
    pub layer3: Option<Layer3Profile>,
    pub working_memory: Option<WorkingMemory>,
}

fn map_profile(row: UserProfileRow) -> Layer3Profile {
    Layer3Profile {
        user_id: row.user_id,
        expertise_domains: row.expertise_domains,
        preferred_answer_style: row.preferred_answer_style,
        frequently_asked_topics: row.frequently_asked_topics,
        custom_preferences: row.custom_preferences,
        inferred_at: row.inferred_at,
        inference_version: row.inference_version,
    }
}

fn map_working_memory(row: DialogueStateRow) -> WorkingMemory {
    WorkingMemory {
        session_id: row.session_id,
        state_type: row.state_type,
        current_topic: row.current_topic,
        pending_questions: row.pending_questions,
        gathered_facts: row.gathered_facts,
        confidence_score: row.confidence_score,
        state_history: row.state_history,
        last_updated_at: row.last_updated_at,
    }
}
