use avrag_llm::ChatMessage;
use common::AppError;

use super::ReActLoop;
use super::config::ModeConfig;
use crate::agents::events::AgentEventSink;
use crate::agents::runtime::AgentRequest;

impl ReActLoop {
    pub(super) async fn prepare_run_request(
        &self,
        mode: &ModeConfig,
        request: AgentRequest,
        _sink: &dyn AgentEventSink,
    ) -> Result<(AgentRequest, usize, u8, contracts::auth_runtime::AuthContext, String), AppError> {
        let loop_user_query = request.query.clone();
        let base_message_count = request
            .messages
            .iter()
            .filter(|turn| turn.role == "user")
            .count()
            + 1;

        let max_iterations = request
            .max_iterations
            .unwrap_or_else(|| {
                mode.budget
                    .resolve_max_iterations(request.metadata.get("user_tier"))
            })
            .max(1);

        let auth: contracts::auth_runtime::AuthContext = serde_json::from_value(request.auth_context.clone())
            .map_err(|e| AppError::internal(format!("invalid auth context: {e}")))?;

        Ok((
            request,
            base_message_count,
            max_iterations,
            auth,
            loop_user_query,
        ))
    }

    pub(super) fn build_initial_messages(
        &self,
        mode: &ModeConfig,
        request: &AgentRequest,
        loop_user_query: &str,
    ) -> Vec<ChatMessage> {
        let _ = mode;
        let mut messages: Vec<ChatMessage> = Vec::new();
        for turn in &request.messages {
            if turn.role == "user" {
                let content = format!("[prior_user_query] {}", turn.content);
                messages.push(ChatMessage::user(&content));
            }
        }
        messages.push(ChatMessage::user(loop_user_query));
        messages
    }
}
