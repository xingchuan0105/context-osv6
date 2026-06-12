use avrag_auth::AuthContext;
use avrag_storage_pg::{PgAppRepository, UserProfileRow};
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
    pub messages: Vec<contracts::chat::ChatMessage>,
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
    pub structured_profile: serde_json::Value,
    pub inferred_at: DateTime<Utc>,
    pub inference_version: String,
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

        Ok(ChatMemoryData {
            layer1: Layer1Messages { messages },
            layer2: summary.map(|value| Layer2Summary {
                session_id,
                summary: value,
                updated_at: Utc::now(),
            }),
            layer3: profile,
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
}

pub struct UserProfileUpdate {
    pub expertise_domains: Vec<String>,
    pub preferred_answer_style: Option<String>,
    pub frequently_asked_topics: Vec<String>,
    pub custom_preferences: serde_json::Value,
    pub structured_profile: serde_json::Value,
    pub inference_version: String,
}

impl ChatMemory {
    pub async fn update_user_profile(
        &self,
        auth: &AuthContext,
        update: UserProfileUpdate,
    ) -> anyhow::Result<()> {
        let Some(user_id) = auth.actor_id().map(|value| value.into_uuid()) else {
            return Ok(());
        };
        let profile = UserProfileRow {
            user_id,
            org_id: auth.org_id(),
            expertise_domains: update.expertise_domains,
            preferred_answer_style: update.preferred_answer_style,
            frequently_asked_topics: update.frequently_asked_topics,
            custom_preferences: update.custom_preferences,
            structured_profile: update.structured_profile,
            inferred_at: Utc::now(),
            inference_version: update.inference_version,
        };
        self.repo.upsert_user_profile(auth, &profile).await?;
        Ok(())
    }
}

pub struct ChatMemoryData {
    pub layer1: Layer1Messages,
    pub layer2: Option<Layer2Summary>,
    pub layer3: Option<Layer3Profile>,
}

fn map_profile(row: UserProfileRow) -> Layer3Profile {
    Layer3Profile {
        user_id: row.user_id,
        expertise_domains: row.expertise_domains,
        preferred_answer_style: row.preferred_answer_style,
        frequently_asked_topics: row.frequently_asked_topics,
        custom_preferences: row.custom_preferences,
        structured_profile: row.structured_profile,
        inferred_at: row.inferred_at,
        inference_version: row.inference_version,
    }
}
