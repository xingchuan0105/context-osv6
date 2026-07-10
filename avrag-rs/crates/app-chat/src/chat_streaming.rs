use common::AppError;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use agent_loop::events::{AgentEvent, AgentEventSink};
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
    sink: &agent_loop::sse_sink::SseSink,
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
    // The terminal `done` event carries the full ChatResponse so clients can finalize.
    // Three fields carry large payloads that are redundant at done time:
    //   - `citations[].content` (full chunk text) — already in the `citations` SSE event
    //   - `tool_results[].data` (raw retrieval output, all chunks) — internal/debug only
    //   - `sources[].snippet` (search result excerpts)
    // Together these can be ~1.5 MB, landing in a single HTTP chunked frame that exceeds
    // the client's stream-read window — so the done event is dropped intermittently
    // (verified via tcpdump: server emits done, client reqwest never delivers it).
    // Strip them here; clients that need citation content already have it from the
    // `citations` event. Structural fields (tool name/status, citation ids, scores) stay.
    let mut trimmed = response.clone();
    for citation in &mut trimmed.citations {
        citation.content = None;
    }
    for tool_result in &mut trimmed.tool_results {
        tool_result.data = None;
    }
    for source in &mut trimmed.sources {
        source.snippet = None;
    }
    serde_json::to_value(&trimmed).unwrap_or_else(|_| serde_json::json!({}))
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

    /// Write-lane streaming entry (SSE).
    pub async fn execute_write_stream(
        &self,
        req: contracts::chat::ChatRequest,
        request_id: String,
        sender: UnboundedSender<ChatEvent>,
        token: CancellationToken,
    ) -> Result<(), AppError> {
        if req.query.trim().is_empty() {
            return Err(AppError::validation("query_required", "query is required"));
        }

        crate::chat::execute_write_pipeline_stream(self.clone(), req, request_id, sender, token)
            .await
    }
}
