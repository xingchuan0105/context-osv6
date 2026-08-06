use std::sync::Arc;

use avrag_llm::{ChatMessage, LlmClient, LlmUsage};

/// Session-wide prefix (before window body). Authored under `prompts/pipeline/`.
pub(crate) const INTERACTION_SESSION_SYSTEM: &str =
    include_str!("../../../../prompts/pipeline/interaction-session.system.md");

/// Result of one session turn (seed or follow-up).
#[derive(Debug, Clone)]
pub(crate) struct SessionTurn {
    pub(crate) content: String,
    pub(crate) usage: LlmUsage,
}

/// Build messages for one turn.
///
/// **DashScope session cache key includes `instructions` (system message).**
/// Within a window the system string must stay identical across seed and produce
/// (body + hint). Stage instructions live only in the user message.
fn build_turn_messages(system_with_body: &str, user: &str) -> Vec<ChatMessage> {
    vec![
        ChatMessage::system(system_with_body.to_string()),
        ChatMessage::user(user.to_string()),
    ]
}

/// Compose system = interaction hint + window body (design 2026-08-06).
pub(crate) fn compose_window_system(window_body: &str) -> String {
    format!("{INTERACTION_SESSION_SYSTEM}{window_body}")
}

/// A single LLM conversation chain for one document **window**.
///
/// `seed` opens the chain; `produce` continues with the **same** system string
/// so provider session cache can hit.
#[derive(Debug, Clone)]
pub(crate) struct DocumentIngestionSession {
    llm: Arc<LlmClient>,
    /// Fixed system for this window (hint + body). Set on first seed.
    system_with_body: Option<String>,
    previous_response_id: Option<String>,
    total_tokens: u64,
}

impl DocumentIngestionSession {
    pub(crate) fn new(llm: Arc<LlmClient>) -> Self {
        Self {
            llm,
            system_with_body: None,
            previous_response_id: None,
            total_tokens: 0,
        }
    }

    /// First turn: establish system (body) + user task.
    pub(crate) async fn seed(
        &mut self,
        system_with_body: &str,
        user: &str,
        temperature: Option<f32>,
    ) -> anyhow::Result<SessionTurn> {
        self.system_with_body = Some(system_with_body.to_string());
        self.complete(None, user, temperature).await
    }

    /// Follow-up turn: same system as seed, new user task (e.g. triplets).
    pub(crate) async fn produce(
        &mut self,
        user: &str,
        temperature: Option<f32>,
    ) -> anyhow::Result<SessionTurn> {
        let previous_response_id = self.previous_response_id.clone();
        self.complete(previous_response_id.as_deref(), user, temperature)
            .await
    }

    async fn complete(
        &mut self,
        previous_response_id: Option<&str>,
        user: &str,
        temperature: Option<f32>,
    ) -> anyhow::Result<SessionTurn> {
        let system = self
            .system_with_body
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("session seed required before produce"))?;
        let messages = build_turn_messages(system, user);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turn_messages_keep_system_constant_and_user_task() {
        let system = compose_window_system("正文段落");
        let messages = build_turn_messages(&system, "阶段任务 JSON");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "system");
        assert!(messages[0].content.contains("正文段落"));
        assert!(messages[0].content.contains("单文档摄取会话"));
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[1].content, "阶段任务 JSON");
        assert!(!messages[0].content.contains("阶段任务"));
    }
}
