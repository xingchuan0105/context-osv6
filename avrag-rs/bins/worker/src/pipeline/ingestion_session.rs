use std::sync::Arc;

use avrag_llm::{ChatMessage, LlmClient, LlmUsage};

/// Session-wide guidance shared by every ingestion turn. Authored under
/// `prompts/pipeline/`, never inline.
pub(crate) const INTERACTION_SESSION_SYSTEM: &str =
    include_str!("../../../../prompts/pipeline/interaction-session.system.md");

/// Result of one session turn (seed or follow-up).
#[derive(Debug, Clone)]
pub(crate) struct SessionTurn {
    pub(crate) content: String,
    pub(crate) usage: LlmUsage,
}

/// A single LLM conversation chain for one document's ingestion.
///
/// `seed` opens the chain without a `previous_response_id`; every follow-up
/// `produce` continues the same chain through the provider's session cache.
/// The session deliberately bypasses the result-level completion cache so the
/// chain stays continuous and every turn can hit the provider-side cache.
#[derive(Debug, Clone)]
pub(crate) struct DocumentIngestionSession {
    llm: Arc<LlmClient>,
    previous_response_id: Option<String>,
    total_tokens: u64,
}

impl DocumentIngestionSession {
    pub(crate) fn new(llm: Arc<LlmClient>) -> Self {
        Self {
            llm,
            previous_response_id: None,
            total_tokens: 0,
        }
    }

    /// First turn of the chain: no previous response to continue from.
    pub(crate) async fn seed(
        &mut self,
        system_prompts: &[&str],
        user: &str,
        temperature: Option<f32>,
    ) -> anyhow::Result<SessionTurn> {
        self.complete(None, system_prompts, user, temperature).await
    }

    /// Follow-up turn: continues the same session chain.
    pub(crate) async fn produce(
        &mut self,
        system_prompts: &[&str],
        user: &str,
        temperature: Option<f32>,
    ) -> anyhow::Result<SessionTurn> {
        let previous_response_id = self.previous_response_id.clone();
        self.complete(
            previous_response_id.as_deref(),
            system_prompts,
            user,
            temperature,
        )
        .await
    }

    async fn complete(
        &mut self,
        previous_response_id: Option<&str>,
        system_prompts: &[&str],
        user: &str,
        temperature: Option<f32>,
    ) -> anyhow::Result<SessionTurn> {
        let mut messages = system_prompts
            .iter()
            .map(|prompt| ChatMessage::system(*prompt))
            .collect::<Vec<_>>();
        messages.push(ChatMessage::user(user));
        let (response, next_id) = self
            .llm
            .complete_response(previous_response_id, &messages, temperature)
            .await?;
        self.previous_response_id = next_id;
        self.total_tokens = self
            .total_tokens
            .saturating_add(u64::from(response.usage.total_tokens));
        Ok(SessionTurn {
            content: response.content,
            usage: response.usage,
        })
    }
}