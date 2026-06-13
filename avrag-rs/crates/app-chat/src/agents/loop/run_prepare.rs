use avrag_llm::ChatMessage;
use common::AppError;

use super::config::ModeConfig;
use super::query_normalize;
use super::ReActLoop;
use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::runtime::AgentRequest;

impl ReActLoop {
    pub(super) async fn prepare_run_request(
        &self,
        mode: &ModeConfig,
        request: AgentRequest,
        norm: query_normalize::NormalizeResult,
        sink: &dyn AgentEventSink,
    ) -> Result<
        (
            AgentRequest,
            usize,
            u8,
            avrag_auth::AuthContext,
            String,
        ),
        AppError,
    > {
        let request = request.with_resolved_query(norm.resolved_query.clone(), norm.meta);
        let slots: Vec<String> = request
            .query_resolution
            .as_ref()
            .map(|meta| {
                meta.slots
                    .iter()
                    .map(|s| {
                        serde_json::to_string(s)
                            .unwrap_or_default()
                            .trim_matches('"')
                            .to_string()
                    })
                    .collect()
            })
            .unwrap_or_default();
        let _ = sink
            .emit(AgentEvent::QueryResolved {
                raw: request.query.clone(),
                resolved: request.effective_query().to_string(),
                slots,
            })
            .await;

        let loop_user_query = if mode.id == "rag" || mode.id == "search" {
            request.effective_query().to_string()
        } else {
            request.query.clone()
        };
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

        let auth: avrag_auth::AuthContext = serde_json::from_value(request.auth_context.clone())
            .map_err(|e| AppError::internal(format!("invalid auth context: {e}")))?;

        Ok((request, base_message_count, max_iterations, auth, loop_user_query))
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
