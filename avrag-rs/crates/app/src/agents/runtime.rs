use super::AgentKind;
use crate::agents::events::AgentEventSink;
use common::{ChatTurnInput, Citation, DegradeTraceItem};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Request context passed to any agent implementation.
/// Concrete agents may downcast or extend this with agent-specific fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    /// Canonical agent kind (already normalized from user input).
    pub kind: AgentKind,
    /// User's natural language query.
    pub query: String,
    /// Notebook / workspace context.
    pub notebook_id: Option<String>,
    /// Session ID for continuity.
    pub session_id: Option<String>,
    /// Document scope for RAG (server-controlled, never expanded by model).
    pub doc_scope: Vec<String>,
    /// Recent message history.
    pub messages: Vec<ChatTurnInput>,
    /// Optional session summary (layer-2 memory).
    pub session_summary: Option<String>,
    /// User preference memory (layer-3 memory).
    pub user_preferences: Option<serde_json::Value>,
    /// Working memory / dialogue state (layer-1 memory).
    pub working_memory: Option<serde_json::Value>,
    /// Debug flag: when true, agents emit DebugTrace events.
    pub debug: bool,
    /// Stream flag: when true, the caller expects incremental events via sink.
    pub stream: bool,
    /// Auth / org context serialized as JSON to avoid leaking auth types into agent layer.
    pub auth_context: serde_json::Value,
    /// Free-form metadata (e.g. source_type, source_token for shared KB).
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
}

/// Result of a completed agent run.
/// Streaming paths accumulate this from sink events; non-streaming paths return it directly.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentRunResult {
    pub answer: String,
    #[serde(default)]
    pub answer_blocks: Vec<contracts::chat::AnswerBlock>,
    #[serde(default)]
    pub citations: Vec<Citation>,
    #[serde(default)]
    pub sources: Vec<common::SourceRef>,
    #[serde(default)]
    pub reasoning_summary: Option<String>,
    #[serde(default)]
    pub degrade_trace: Vec<DegradeTraceItem>,
    #[serde(default)]
    pub usage: Option<AgentRunUsage>,
    #[serde(default)]
    pub debug_payload: Option<serde_json::Value>,
    #[serde(default)]
    pub message_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRunUsage {
    pub provider: String,
    pub model: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub request_count: u64,
}

/// Trait for concrete agent implementations.
/// Each agent (Chat, WebSearch, Rag) implements this trait.
#[async_trait::async_trait]
pub trait Agent: Send + Sync {
    /// Run the agent, emitting events via the provided sink.
    /// The returned `AgentRunResult` must match the final state represented by sink events.
    async fn run(
        &self,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, common::AppError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::events::{AgentEvent, CollectingSink};

    #[test]
    fn test_agent_request_serde_roundtrip() {
        let req = AgentRequest {
            kind: AgentKind::Chat,
            query: "hello".to_string(),
            notebook_id: Some("nb-1".to_string()),
            session_id: Some("sess-1".to_string()),
            doc_scope: vec!["doc-1".to_string()],
            messages: vec![],
            session_summary: None,
            user_preferences: None,
            working_memory: None,
            debug: false,
            stream: false,
            auth_context: serde_json::json!({"org_id": "o1"}),
            metadata: BTreeMap::new(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: AgentRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req.kind, parsed.kind);
        assert_eq!(req.query, parsed.query);
    }

    #[test]
    fn test_agent_run_result_default() {
        let result = AgentRunResult::default();
        assert!(result.answer.is_empty());
        assert!(result.citations.is_empty());
        assert!(result.degrade_trace.is_empty());
        assert!(result.reasoning_summary.is_none());
    }

    #[tokio::test]
    async fn test_collecting_sink_in_runtime_context() {
        let sink = CollectingSink::new();
        sink.emit(AgentEvent::MessageDelta {
            text: "Hello".to_string(),
        })
        .await;
        sink.emit(AgentEvent::Done {
            final_message: Some("Hello".to_string()),
            usage: None,
        })
        .await;
        let events = sink.into_events();
        assert_eq!(events.len(), 2);
    }
}
