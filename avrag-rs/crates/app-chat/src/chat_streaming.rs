use common::AppError;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::context::ChatContext;
use contracts::chat::ChatEvent;
pub const STREAM_PLACEHOLDER_MESSAGE_ID: i64 = 0;
const STREAM_TOKEN_CHUNK_CHARS: usize = 24;

pub fn stream_event_message_id(message_id: Option<i64>) -> i64 {
    message_id.unwrap_or(STREAM_PLACEHOLDER_MESSAGE_ID)
}

pub fn chunk_text_for_stream(text: &str) -> Vec<String> {
    let chars = text.chars().collect::<Vec<_>>();

    if chars.is_empty() {
        return Vec::new();
    }

    chars
        .chunks(STREAM_TOKEN_CHUNK_CHARS)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect()
}

pub async fn emit_buffered_agent_answer_if_needed(
    sink: &crate::agents::sse_sink::SseSink,
    answer: &str,
) {
    if sink.has_message_delta() || answer.is_empty() {
        return;
    }

    for chunk in chunk_text_for_stream(answer) {
        let _ = sink.emit(AgentEvent::MessageDelta { text: chunk }).await;
    }
}

pub fn chat_done_payload(response: &contracts::chat::ChatResponse) -> serde_json::Value {
    serde_json::to_value(response).unwrap_or_else(|_| serde_json::json!({}))
}

impl ChatContext {
    pub async fn execute_chat_stream(
        &self,
        req: contracts::chat::ChatRequest,
        request_id: String,
        sender: UnboundedSender<ChatEvent>,
        token: CancellationToken,
    ) -> Result<(), AppError> {
        if req.query.trim().is_empty() {
            return Err(AppError::validation("query_required", "query is required"));
        }

        crate::chat::execute_chat_pipeline_stream(self.clone(), req, request_id, sender, token)
            .await
    }
}
