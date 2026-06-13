use contracts::chat::Citation;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Re-export audit record type for event embedding.
pub use app_documents::AuditRecord;

/// Internal event emitted by agents during execution.
/// These events are mapped to `ChatEvent` for SSE transport.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum AgentEvent {
    /// High-level progress / activity notification.
    Activity { stage: String, message: String },
    /// Incremental reasoning summary text (e.g. from model's reasoning tokens).
    ReasoningSummaryDelta { text: String },
    /// Incremental answer/message text.
    MessageDelta { text: String },
    /// Debug trace event, gated by debug flag.
    DebugTrace {
        kind: String,
        payload: serde_json::Value,
    },
    /// Citations discovered or validated.
    Citations { citations: Vec<Citation> },
    /// Usage telemetry (tokens, request count, provider, model).
    Usage {
        provider: String,
        model: String,
        prompt_tokens: u64,
        completion_tokens: u64,
        total_tokens: u64,
        request_count: u64,
        #[serde(default)]
        metadata: BTreeMap<String, serde_json::Value>,
    },
    /// Final completion event.
    Done {
        #[serde(default)]
        final_message: Option<String>,
        #[serde(default)]
        usage: Option<AgentUsage>,
    },
    /// Terminal error event.
    Error { code: String, message: String },
    /// Plan/Decompose phase decision output (white-box observability).
    PlanDecision {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        selected_tools: Vec<contracts::ToolCall>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        selected_skills: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        selected_writing_styles: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        behavior_mode: Option<String>,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        reasoning: String,
    },
    /// Tool execution result (white-box observability).
    ToolResult {
        tool: String,
        status: contracts::ToolStatus,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        data: Option<serde_json::Value>,
        elapsed_ms: u64,
    },
    /// Evaluation phase output (white-box observability).
    Evaluation {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signals: Option<serde_json::Value>,
        decision: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        reasoning: String,
    },
    /// Budget consumption tick (white-box observability).
    BudgetTick { current: u8, max: u8 },
    /// Routing decision event (white-box observability).
    /// Emitted when the router resolves a mode for the request.
    RoutingDecision {
        mode_id: String,
        matched_rule: String,
        confidence: f64,
        explanation: String,
    },
    /// Terminal decision event (white-box observability).
    /// Emitted when the agent reaches a final decision.
    Terminal {
        decision: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// Trace summary event (white-box observability).
    /// Emitted at the end of a run with complete trace information.
    TraceSummary {
        trace_id: String,
        total_elapsed_ms: u64,
    },
    /// Audit record emitted at key security/policy decision points.
    /// Collected by the orchestrator for persistence into the audit log.
    Audit { record: AuditRecord },
    /// ADR-0008: ReAct turn started.
    TurnStart { iteration: u8, phase: String },
    /// ADR-0008: ReAct turn ended.
    TurnEnd { iteration: u8, exit_reason: String },
    /// ADR-0008: Query normalization completed.
    QueryResolved {
        raw: String,
        resolved: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        slots: Vec<String>,
    },
    /// ADR-0008: Synthesis JSON contract active.
    SynthesisContract { schema_version: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentUsage {
    pub provider: String,
    pub model: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    #[serde(default)]
    pub cached_tokens: u64,
}

/// Sink for emitting agent events during a run.
/// Both streaming and non-streaming paths use the same sink abstraction:
/// - Streaming: forwards events immediately over SSE.
/// - Non-streaming: collects events into a Vec, then returns final result.
#[async_trait::async_trait]
pub trait AgentEventSink: Send + Sync {
    /// Emit an event. Returns Err(()) if the sink is closed (e.g. client disconnected).
    async fn emit(&self, event: AgentEvent) -> Result<(), ()>;
    /// Return a boxed clone of this sink so contexts can take ownership.
    /// For channel-backed sinks the clone shares the same channel;
    /// for collecting sinks the clone shares the same buffer.
    fn clone_boxed(&self) -> Box<dyn AgentEventSink>;
}

/// A no-op sink that discards all events.
pub struct NoopSink;

#[async_trait::async_trait]
impl AgentEventSink for NoopSink {
    async fn emit(&self, _event: AgentEvent) -> Result<(), ()> {
        Ok(())
    }

    fn clone_boxed(&self) -> Box<dyn AgentEventSink> {
        Box::new(NoopSink)
    }
}

/// A collecting sink that accumulates events into an internal buffer.
/// Used by the non-streaming path to gather events for final response assembly.
pub struct CollectingSink {
    events: std::sync::Arc<std::sync::Mutex<Vec<AgentEvent>>>,
}

impl Default for CollectingSink {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for CollectingSink {
    fn clone(&self) -> Self {
        Self {
            events: std::sync::Arc::clone(&self.events),
        }
    }
}

impl CollectingSink {
    pub fn new() -> Self {
        Self {
            events: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    pub fn into_events(self) -> Vec<AgentEvent> {
        match std::sync::Arc::try_unwrap(self.events) {
            Ok(mutex) => mutex.into_inner().unwrap_or_default(),
            Err(arc) => arc.lock().map(|g| g.clone()).unwrap_or_default(),
        }
    }

    pub fn events(&self) -> Vec<AgentEvent> {
        match self.events.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

#[async_trait::async_trait]
impl AgentEventSink for CollectingSink {
    async fn emit(&self, event: AgentEvent) -> Result<(), ()> {
        if let Ok(mut guard) = self.events.lock() {
            guard.push(event);
            Ok(())
        } else {
            Err(())
        }
    }

    fn clone_boxed(&self) -> Box<dyn AgentEventSink> {
        Box::new(self.clone())
    }
}

/// A sink that wraps an `UnboundedSender` for immediate forwarding.
/// Used by the streaming path to push events to the SSE coalescer.
pub struct ChannelSink<T> {
    sender: tokio::sync::mpsc::UnboundedSender<T>,
}

impl<T> ChannelSink<T> {
    pub fn new(sender: tokio::sync::mpsc::UnboundedSender<T>) -> Self {
        Self { sender }
    }
}

#[async_trait::async_trait]
impl AgentEventSink for ChannelSink<AgentEvent> {
    async fn emit(&self, event: AgentEvent) -> Result<(), ()> {
        self.sender.send(event).map_err(|_| ())
    }

    fn clone_boxed(&self) -> Box<dyn AgentEventSink> {
        Box::new(ChannelSink::new(self.sender.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_event_serde_roundtrip() {
        let events = vec![
            AgentEvent::Activity {
                stage: "planning".to_string(),
                message: "Building retrieval plan".to_string(),
            },
            AgentEvent::ReasoningSummaryDelta {
                text: "The user wants to know".to_string(),
            },
            AgentEvent::MessageDelta {
                text: "Here is the answer".to_string(),
            },
            AgentEvent::DebugTrace {
                kind: "retrieval".to_string(),
                payload: serde_json::json!({"channel": "dense"}),
            },
            AgentEvent::Citations { citations: vec![] },
            AgentEvent::Usage {
                provider: "openai".to_string(),
                model: "gpt-4".to_string(),
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
                request_count: 1,
                metadata: BTreeMap::new(),
            },
            AgentEvent::Done {
                final_message: Some("Done".to_string()),
                usage: Some(AgentUsage {
                    provider: "openai".to_string(),
                    model: "gpt-4".to_string(),
                    prompt_tokens: 100,
                    completion_tokens: 50,
                    total_tokens: 150,
                    cached_tokens: 0,
                }),
            },
            AgentEvent::Error {
                code: "E001".to_string(),
                message: "Something failed".to_string(),
            },
            AgentEvent::Audit {
                record: app_documents::AuditRecord {
                    audit_id: "a1".to_string(),
                    org_id: "o1".to_string(),
                    actor_id: Some("u1".to_string()),
                    action: app_documents::AuditAction::RoutingDecision,
                    resource_type: "agent_request".to_string(),
                    resource_id: "r1".to_string(),
                    payload: serde_json::json!({"mode_id": "rag"}),
                    created_at: "2024-01-01T00:00:00Z".to_string(),
                },
            },
            AgentEvent::PlanDecision {
                selected_tools: vec![contracts::ToolCall {
                    tool: "dense_retrieval".to_string(),
                    version: "1.0".to_string(),
                    args: serde_json::json!({"query": "test"}),
                }],
                selected_skills: vec!["rag-plan".to_string()],
                selected_writing_styles: vec![],
                behavior_mode: None,
                reasoning: "selected based on query type".to_string(),
            },
            AgentEvent::ToolResult {
                tool: "web_search".to_string(),
                status: contracts::ToolStatus::Ok,
                data: Some(serde_json::json!({"results": 5})),
                elapsed_ms: 1234,
            },
            AgentEvent::Evaluation {
                signals: Some(serde_json::json!({"recall": 10})),
                decision: "synthesize".to_string(),
                reasoning: "sufficient evidence".to_string(),
            },
            AgentEvent::BudgetTick { current: 1, max: 3 },
        ];

        for event in events {
            let json = serde_json::to_string(&event).unwrap();
            let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
            let reparsed = serde_json::to_string(&parsed).unwrap();
            assert_eq!(json, reparsed, "serde roundtrip failed for {:?}", event);
        }
    }

    #[tokio::test]
    async fn test_noop_sink() {
        let sink = NoopSink;
        let _ = sink
            .emit(AgentEvent::MessageDelta {
                text: "hello".to_string(),
            })
            .await;
        // No panic, no collection.
    }

    #[tokio::test]
    async fn test_collecting_sink() {
        let sink = CollectingSink::new();
        let _ = sink
            .emit(AgentEvent::Activity {
                stage: "plan".to_string(),
                message: "planning".to_string(),
            })
            .await;
        let _ = sink
            .emit(AgentEvent::MessageDelta {
                text: "answer".to_string(),
            })
            .await;
        let events = sink.events();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], AgentEvent::Activity { .. }));
        assert!(matches!(events[1], AgentEvent::MessageDelta { .. }));
    }
}
